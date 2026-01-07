#!/usr/bin/perl
use strict;
use warnings;
use utf8;
use open qw(:std :encoding(UTF-8));
use Encode qw(decode encode);

use XML::LibXML;
use HTML::TreeBuilder;
use LWP::UserAgent;
use URI;
use MIME::Base64;
use JSON;
use File::Basename;
use Digest::MD5 qw(md5_hex);
use Digest::SHA qw(sha256_hex);
use Time::Piece;
use Getopt::Long;

# Ensure UTF-8 output
binmode(STDOUT, ':encoding(UTF-8)');
binmode(STDERR, ':encoding(UTF-8)');

# Try to use UUID module, fall back to manual generation
my $HAS_UUID_MODULE;
BEGIN {
    eval { require Data::UUID; $HAS_UUID_MODULE = 'Data::UUID'; 1; }
    or eval { require UUID::Tiny; $HAS_UUID_MODULE = 'UUID::Tiny'; 1; }
    or $HAS_UUID_MODULE = 0;
}

# ============================= Configuration =============================

my $OPENAI_API_BASE = $ENV{OPENAI_API_BASE} || 'https://api.openai.com/v1';
my $OPENAI_API_KEY  = $ENV{OPENAI_API_KEY} || '';
my $OPENAI_MODEL    = $ENV{OPENAI_MODEL} || 'gpt-4o-mini';

my %opts = (
    'ai-tags'          => 0,
    'resolve-urls'     => 1,
    'github-readme'    => 1,
    'fetch-summaries'  => 1,
    'max-notes'        => 0,
    'output'           => '',
);

GetOptions(
    'ai-tags'           => \$opts{'ai-tags'},
    'resolve-urls!'     => \$opts{'resolve-urls'},
    'github-readme!'    => \$opts{'github-readme'},
    'fetch-summaries!'  => \$opts{'fetch-summaries'},
    'max-notes=i'       => \$opts{'max-notes'},
    'output=s'          => \$opts{'output'},
    'help'              => \&usage,
) or usage();

my @input_files = @ARGV or usage();

if ($opts{'ai-tags'} && !$OPENAI_API_KEY) {
    die "OPENAI_API_KEY environment variable required for AI tagging\n";
}

# ================================ Logging =================================

sub log_msg {
    my ($msg) = @_;
    my $ts = localtime->strftime('%Y-%m-%d %H:%M:%S');
    print STDERR "[$ts] $msg\n";
}

sub usage {
    print STDERR <<'USAGE';
Usage: enex2jottery.pl [OPTIONS] input.enex [input2.enex ...]

Convert Evernote ENEX files to Jottery JSON format.
Supports multiple files and wildcards (e.g., notebooks/*.enex)

Options:
    --ai-tags              Enable AI-based tag generation (requires OPENAI_API_KEY)
    --no-resolve-urls      Disable URL resolution (ift.tt, etc.)
    --no-github-readme     Disable GitHub README fetching
    --no-fetch-summaries   Disable content summary fetching for URLs
    --max-notes N          Process only first N notes per file (0 = all)
    --output FILE          Output file (default: combined.json or input.json)
    --help                 Show this help

Environment Variables:
    OPENAI_API_KEY      OpenAI/OpenRouter API key (required for --ai-tags)
    OPENAI_API_BASE     API base URL (default: https://api.openai.com/v1)
    OPENAI_MODEL        Model name (default: gpt-4o-mini)

Examples:
    # Single notebook
    enex2jottery.pl notes.enex

    # Multiple notebooks (combined output)
    enex2jottery.pl work.enex personal.enex projects.enex

    # All notebooks with wildcard
    enex2jottery.pl notebooks/*.enex

    # With AI tagging
    OPENAI_API_KEY=sk-... enex2jottery.pl --ai-tags *.enex

    # Custom output
    enex2jottery.pl --output all-notes.json notebooks/*.enex
USAGE
    exit 1;
}

# ============================= HTML to Markdown ============================

sub html_to_markdown {
    my ($html) = @_;

    # Ensure we're working with decoded UTF-8 text
    if (!utf8::is_utf8($html)) {
        $html = decode('UTF-8', $html, Encode::FB_DEFAULT);
    }

    # Build HTML tree
    my $tree = HTML::TreeBuilder->new();
    $tree->parse_content($html);

    my $md = _node_to_markdown($tree);

    $tree->delete;

    # Clean up excessive blank lines and whitespace
    $md =~ s/\n{3,}/\n\n/g;
    $md =~ s/[ \t]+\n/\n/g;
    $md =~ s/^\s+|\s+$//g;

    return $md;
}

sub _node_to_markdown {
    my ($node) = @_;

    # Text node - return as-is
    unless (ref $node) {
        my $text = $node;
        $text =~ s/^\s+|\s+$/ /g;  # Normalize whitespace
        return $text;
    }

    my $tag = $node->tag || '';

    # Skip head, script, style
    return '' if $tag =~ /^(head|script|style)$/;

    # Line breaks
    return "\n" if $tag eq 'br';

    # Horizontal rule
    return "\n---\n" if $tag eq 'hr';

    # Headers
    if ($tag =~ /^h([1-6])$/) {
        my $level = $1;
        my $text = _children_to_markdown($node);
        $text =~ s/^\s+|\s+$//g;
        return "\n" . ('#' x $level) . " $text\n\n" if $text;
        return '';
    }

    # Paragraphs and divs
    if ($tag eq 'p' || $tag eq 'div') {
        my $text = _children_to_markdown($node);
        $text =~ s/^\s+|\s+$//g;
        return "\n$text\n" if $text;
        return '';
    }

    # Lists
    if ($tag eq 'ul' || $tag eq 'ol') {
        my $result = "\n";
        my $counter = 1;
        for my $li ($node->look_down(_tag => 'li')) {
            my $text = _children_to_markdown($li);
            $text =~ s/^\s+|\s+$//g;
            $text =~ s/\n/ /g;  # Flatten multi-line list items
            my $bullet = $tag eq 'ul' ? '-' : "$counter.";
            $result .= "$bullet $text\n";
            $counter++;
        }
        return $result . "\n";
    }

    # Skip list items (handled by ul/ol)
    return '' if $tag eq 'li';

    # Code blocks
    if ($tag eq 'pre') {
        my $code = $node->as_text;
        return "\n```\n$code\n```\n";
    }

    # Inline code
    if ($tag eq 'code') {
        my $code = $node->as_text;
        return "`$code`";
    }

    # Links
    if ($tag eq 'a') {
        my $href = $node->attr('href') || '';
        my $text = _children_to_markdown($node);
        $text =~ s/^\s+|\s+$//g;
        return "[$text]($href)" if $href && $text;
        return $text if $text;
        return '';
    }

    # Bold
    if ($tag eq 'b' || $tag eq 'strong') {
        my $text = _children_to_markdown($node);
        return "**$text**" if $text =~ /\S/;
        return '';
    }

    # Italic
    if ($tag eq 'i' || $tag eq 'em') {
        my $text = _children_to_markdown($node);
        return "*$text*" if $text =~ /\S/;
        return '';
    }

    # Images (already converted to markdown syntax in ENML processing)
    if ($tag eq 'img') {
        my $src = $node->attr('src') || '';
        my $alt = $node->attr('alt') || 'image';
        return "![$alt]($src)" if $src;
        return '';
    }

    # Blockquotes
    if ($tag eq 'blockquote') {
        my $text = _children_to_markdown($node);
        $text =~ s/^\s+|\s+$//g;
        $text =~ s/^/> /gm;
        return "\n$text\n";
    }

    # Tables - simple conversion
    if ($tag eq 'table') {
        my $text = $node->as_text;
        return "\n$text\n";
    }

    # Default: process children
    return _children_to_markdown($node);
}

sub _children_to_markdown {
    my ($node) = @_;
    my $result = '';
    for my $child ($node->content_list) {
        $result .= _node_to_markdown($child);
    }
    return $result;
}

# ============================= URL Processing ==============================

my $ua = LWP::UserAgent->new(
    timeout => 10,
    max_redirect => 5,
    agent => 'Jottery ENEX Converter/1.0',
);

sub resolve_url {
    my ($url) = @_;

    log_msg("Resolving URL: $url");

    my $response = $ua->get($url);
    unless ($response->is_success) {
        log_msg("Failed to resolve: " . $response->status_line);
        return $url;
    }

    my $real_url = $response->request->uri->as_string;
    log_msg("Resolved to: $real_url");

    return $real_url;
}

sub fetch_github_readme {
    my ($url) = @_;

    # Parse GitHub URL: https://github.com/user/repo
    if ($url =~ m{^https?://github\.com/([^/]+)/([^/]+?)/?$}) {
        my ($user, $repo) = ($1, $2);
        $repo =~ s/\.git$//;

        # Try to fetch README.md from main branch
        my $readme_url = "https://raw.githubusercontent.com/$user/$repo/main/README.md";
        log_msg("Fetching GitHub README: $readme_url");

        my $response = $ua->get($readme_url);
        if ($response->is_success) {
            my $content = $response->decoded_content;
            # Ensure UTF-8
            if (!utf8::is_utf8($content)) {
                $content = decode('UTF-8', $content, Encode::FB_DEFAULT);
            }
            return { content => $content, raw_url => $readme_url };
        }

        # Try master branch
        $readme_url = "https://raw.githubusercontent.com/$user/$repo/master/README.md";
        $response = $ua->get($readme_url);
        if ($response->is_success) {
            my $content = $response->decoded_content;
            if (!utf8::is_utf8($content)) {
                $content = decode('UTF-8', $content, Encode::FB_DEFAULT);
            }
            return { content => $content, raw_url => $readme_url };
        }
    }

    return undef;
}

sub extract_content_summary {
    my ($url) = @_;

    return undef unless $opts{'fetch-summaries'};

    log_msg("Fetching content from: $url");

    my $response = $ua->get($url);
    unless ($response->is_success) {
        log_msg("Failed to fetch content: " . $response->status_line);
        return undef;
    }

    my $content = $response->decoded_content;

    # Ensure UTF-8
    if (!utf8::is_utf8($content)) {
        $content = decode('UTF-8', $content, Encode::FB_DEFAULT);
    }

    my $tree = HTML::TreeBuilder->new();
    $tree->parse_content($content);

    # Extract title
    my $title_elem = $tree->look_down(_tag => 'title');
    my $title = $title_elem ? $title_elem->as_text : '';
    $title =~ s/^\s+|\s+$//g;

    # Extract meaningful content as markdown (not just text)
    my $text = '';

    # Try article/main content first - convert to markdown
    for my $tag (qw(article main)) {
        my $elem = $tree->look_down(_tag => $tag);
        if ($elem) {
            $text = _node_to_markdown($elem);
            last;
        }
    }

    # Fallback: try meta description
    if (!$text || length($text) < 100) {
        my $meta = $tree->look_down(_tag => 'meta', name => 'description');
        if ($meta) {
            $text = $meta->attr('content') || '';
        }
    }

    # Fallback: get first few paragraphs as markdown
    if (!$text || length($text) < 100) {
        my @paras = $tree->look_down(_tag => 'p');
        my @para_texts;
        for my $p (@paras) {
            my $p_md = _node_to_markdown($p);
            $p_md =~ s/^\s+|\s+$//g;
            push @para_texts, $p_md if length($p_md) > 50;
            last if @para_texts >= 3;
        }
        $text = join("\n\n", @para_texts);
    }

    $tree->delete;

    # Clean up - remove excessive whitespace but preserve markdown structure
    $text =~ s/\n{3,}/\n\n/g;
    $text =~ s/^\s+|\s+$//g;

    # Truncate if too long
    if (length($text) > 1000) {
        $text = substr($text, 0, 1000) . '...';
    }

    return undef if length($text) < 50;

    return {
        title => $title,
        summary => $text,
    };
}

# ============================ Content Processing ===========================

sub extract_title {
    my ($content) = @_;

    # Try to find first heading
    if ($content =~ /^#\s+(.+)$/m) {
        return $1;
    }

    # Try first non-empty line
    my @lines = split /\n/, $content;
    for my $line (@lines) {
        $line =~ s/^\s+|\s+$//g;
        next if $line eq '';
        next if $line eq '--';
        next if $line =~ /^[\[\(]/; # Skip lines starting with [ or (

        # Truncate if too long
        $line = substr($line, 0, 100) . '...' if length($line) > 100;
        return $line;
    }

    return 'Untitled Note';
}

sub process_urls_in_content {
    my ($content) = @_;

    return $content unless $opts{'resolve-urls'};

    my %url_cache;
    my @additions;

    # Find all markdown links and process them
    my @links;
    while ($content =~ /\[([^\]]+)\]\(([^\)]+)\)/g) {
        my ($text, $url) = ($1, $2);
        next if $url =~ /^attachment:/; # Skip attachment links
        push @links, { text => $text, url => $url };
    }

    for my $link (@links) {
        my ($text, $url) = ($link->{text}, $link->{url});

        next if $url_cache{$url}; # Already processed
        $url_cache{$url} = 1;

        # ONLY process short URL redirectors (ift.tt, bit.ly, etc.)
        # Don't visit any other links - they're often old and dead
        unless ($url =~ m{^https?://(ift\.tt|bit\.ly|tinyurl\.com|goo\.gl|t\.co)/}) {
            next;
        }

        # Resolve short URL to real destination
        my $resolved_url = resolve_url($url);
        if ($resolved_url ne $url) {
            # Replace the URL in the markdown link
            $content =~ s/\Q[$text]($url)\E/[$text]($resolved_url)/g;

            # For resolved GitHub URLs, try to fetch README
            if ($opts{'github-readme'} && $resolved_url =~ m{^https?://github\.com/[^/]+/[^/]+/?$}) {
                my $readme_data = fetch_github_readme($resolved_url);
                if ($readme_data) {
                    my $readme_content = $readme_data->{content};
                    $readme_content = substr($readme_content, 0, 3000) if length($readme_content) > 3000;
                    push @additions, {
                        url => $resolved_url,
                        raw_url => $readme_data->{raw_url},
                        title => "GitHub README",
                        content => $readme_content,
                    };
                    next;
                }
            }

            # Fetch content summary ONLY for resolved short URLs
            if ($opts{'fetch-summaries'}) {
                my $summary_data = extract_content_summary($resolved_url);
                if ($summary_data) {
                    push @additions, {
                        url => $resolved_url,
                        title => $summary_data->{title},
                        content => $summary_data->{summary},
                    };
                }
            }
        }
    }

    # Append summaries and README content
    if (@additions) {
        $content .= "\n\n---\n\n## Linked Content\n\n";
        for my $add (@additions) {
            $content .= "### $add->{title}\n\n";
            $content .= "Source: $add->{url}\n\n";
            # For GitHub, include link to raw README
            if ($add->{raw_url}) {
                $content .= "Raw: $add->{raw_url}\n\n";
            }
            $content .= "$add->{content}\n\n";
        }
    }

    return $content;
}

# ============================== AI Tagging =================================

sub generate_ai_title_and_tags {
    my ($content, $original_title) = @_;

    return (undef, ()) unless $opts{'ai-tags'};

    my $json = JSON->new->utf8;

    # Truncate content for API
    my $sample = substr($content, 0, 2500);

    my $request = {
        model => $OPENAI_MODEL,
        temperature => 0.3,
        messages => [
            {
                role => 'system',
                content => <<'PROMPT'
Analyse this note and generate:
1. A concise, descriptive title (max 80 chars) that captures the essence of the note
2. 1-3 semantic tags in order of specificity (broad category → subcategory → specific topic)

Rules for title:
- Should be clear and descriptive
- No markdown formatting (just plain text)
- If the note is about a link/article, summarise what it's about
- If original title is meaningful, you can refine it
- Use British English spellings (e.g., "organise", "colour", "behaviour", "favourite")

Rules for tags:
- Lowercase only
- Use hyphens for multi-word tags (e.g., "machine-learning")
- Be concise (1-3 words per tag)
- Order from broad to specific (e.g., ["technology", "programming", "rust-lang"])
- Use British English spellings (e.g., "organisation" not "organization")

Return ONLY valid JSON in this exact format:
{"title": "Your Title Here", "tags": ["broad-tag", "specific-tag"]}
PROMPT
            },
            {
                role => 'user',
                content => "Original title: $original_title\n\nContent:\n$sample"
            }
        ]
    };

    my $req = HTTP::Request->new(POST => "$OPENAI_API_BASE/chat/completions");
    $req->header('Content-Type' => 'application/json');
    $req->header('Authorization' => "Bearer $OPENAI_API_KEY");
    $req->content($json->encode($request));

    log_msg("Requesting AI title and tags...");

    my $response = $ua->request($req);
    unless ($response->is_success) {
        log_msg("AI request failed: " . $response->status_line);
        return (undef, ());
    }

    my $data = $json->decode($response->decoded_content);
    my $ai_response = $data->{choices}[0]{message}{content};

    # Parse JSON from response
    $ai_response =~ s/^```(?:json)?//;
    $ai_response =~ s/```$//;
    $ai_response =~ s/^\s+|\s+$//g;

    my $result = eval { $json->decode($ai_response) };
    if ($@ || ref $result ne 'HASH') {
        log_msg("AI response parse error: $@");
        return (undef, ());
    }

    my $title = $result->{title} || undef;
    my @tags = ref $result->{tags} eq 'ARRAY' ? @{$result->{tags}} : ();

    # Sanitize tags
    @tags = map { sanitize_tag($_) } grep { defined && length } @tags;
    @tags = grep { length } @tags;

    log_msg("AI generated: title='$title', tags=[" . join(', ', @tags) . "]");

    return ($title, @tags);
}

# ============================= ENEX Parsing ================================

sub parse_enex {
    my ($file) = @_;

    log_msg("Reading ENEX file: $file");

    # Configure parser to avoid network access and DTD issues
    my $parser = XML::LibXML->new();
    $parser->set_options(
        no_network => 1,           # Don't access network for DTDs
        load_ext_dtd => 0,          # Don't load external DTDs
        expand_entities => 1,       # Expand entities
        recover => 2,               # Recover from errors
        suppress_errors => 1,       # Suppress error messages
        suppress_warnings => 1,     # Suppress warnings
    );

    my $doc = $parser->parse_file($file);

    my @note_nodes = $doc->findnodes('//note');
    my $total = scalar @note_nodes;

    log_msg("Found $total notes");

    if ($opts{'max-notes'} && $total > $opts{'max-notes'}) {
        @note_nodes = @note_nodes[0 .. $opts{'max-notes'} - 1];
        log_msg("Processing first $opts{'max-notes'} notes");
    }

    return @note_nodes;
}

sub parse_evernote_date {
    my ($date_str) = @_;
    # Format: 20250106T123045Z
    if ($date_str =~ /^(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})Z$/) {
        return "$1-$2-$3T$4:$5:$6.000Z";
    }
    return gmtime->datetime . 'Z';
}

sub process_note {
    my ($note_node, $index, $total, $notebook_tag) = @_;

    # Extract basic fields
    my $title = $note_node->findvalue('./title') || 'Untitled';
    my $created = parse_evernote_date($note_node->findvalue('./created'));
    my $updated = parse_evernote_date($note_node->findvalue('./updated'));

    log_msg("[$index/$total] Processing: $title");

    # Extract ENML content
    my $content_node = $note_node->findnodes('./content')->[0];
    return undef unless $content_node;

    my $enml = $content_node->textContent;
    $enml =~ s/^<!\[CDATA\[//;
    $enml =~ s/\]\]>$//;
    $enml =~ s/<\/?en-note[^>]*>//g;

    # Process resources (attachments) - build list with filenames
    my @resource_nodes = $note_node->findnodes('./resource');
    my @attachments;
    my %hash_to_index;  # Map hash to attachment index
    my %used_filenames; # Track used filenames to ensure uniqueness

    my $att_index = 0;
    for my $res (@resource_nodes) {
        my $mime = $res->findvalue('./mime') || 'application/octet-stream';
        my $data_node = $res->findnodes('./data')->[0];
        next unless $data_node;

        my $data = $data_node->textContent;
        my $hash = $data_node->getAttribute('hash') || '';

        # Generate unique hash for this attachment (use data hash or generate from content)
        my $content_hash = $hash || substr(sha256_hex($data), 0, 8);
        $content_hash = substr($content_hash, 0, 8);

        # Try to get original filename from resource-attributes
        my $orig_filename = $res->findvalue('./resource-attributes/file-name') || '';

        # Get proper extension from MIME type
        my $ext = guess_extension($mime);

        # Generate filename - use original if available, otherwise generate unique one
        my $filename;
        if ($orig_filename && $orig_filename ne 'Attachment') {
            # Sanitize original filename but preserve unicode
            $orig_filename =~ s/[\/\\:*?"<>|]/_/g;  # Remove filesystem-unsafe chars only
            $orig_filename =~ s/\s+/_/g;  # Replace spaces with underscores

            # Ensure correct extension
            if ($orig_filename !~ /\.\w+$/) {
                $orig_filename .= ".$ext";
            }
            $filename = $orig_filename;
        } else {
            # No useful filename - generate one with hash
            $filename = "attachment-$content_hash.$ext";
        }

        # Ensure filename is unique
        if ($used_filenames{$filename}) {
            my $base = $filename;
            $base =~ s/\.[^.]+$//;
            $filename = "$base-$content_hash.$ext";
        }
        $used_filenames{$filename} = 1;

        # Map hash to this attachment index (if hash exists)
        $hash_to_index{$hash} = $att_index if $hash;

        push @attachments, {
            filename => $filename,
            mimeType => $mime,
            data => $data,
        };
        $att_index++;
    }

    # Replace <en-media> tags with markdown - use hash lookup or sequential fallback
    my $seq_index = 0;
    $enml =~ s{<en-media\s+([^>]+?)\s*/?>}{
        my $attrs = $1;
        my ($hash) = $attrs =~ /hash="([^"]+)"/;
        my ($type) = $attrs =~ /type="([^"]+)"/;

        # Find the attachment - try hash first, fall back to sequential
        my $idx;
        if ($hash && exists $hash_to_index{$hash}) {
            $idx = $hash_to_index{$hash};
        } else {
            $idx = $seq_index;
        }
        $seq_index++;

        my $filename = ($idx < @attachments) ? $attachments[$idx]{filename} : "missing-$seq_index";

        if ($type && $type =~ /^image\//) {
            "![attachment](attachment:$filename)";
        } else {
            "[Attachment: $filename](attachment:$filename)";
        }
    }ige;

    # Convert to Markdown
    my $content = html_to_markdown($enml);

    # If HTML conversion failed, try simple text extraction
    if (!$content || $content !~ /\S/) {
        $content = $enml;
        $content =~ s/<[^>]+>//g;  # Strip all HTML tags
        $content =~ s/&nbsp;/ /g;
        $content =~ s/&amp;/&/g;
        $content =~ s/&lt;/</g;
        $content =~ s/&gt;/>/g;
        $content =~ s/&quot;/"/g;
        $content =~ s/^\s+|\s+$//g;
    }

    # Process URLs
    $content = process_urls_in_content($content);

    # Extract meaningful title (use note title if content extraction fails)
    my $actual_title = extract_title($content);
    if ($actual_title eq 'Untitled Note' && $title ne 'Untitled') {
        $actual_title = $title;
    }

    # Generate tags (and optionally AI title)
    my @tags = ($notebook_tag);
    my $ai_title;

    if ($opts{'ai-tags'}) {
        ($ai_title, my @ai_tags) = generate_ai_title_and_tags($content, $title);
        push @tags, @ai_tags;

        # Use AI title if we got one
        if ($ai_title && length($ai_title) > 0) {
            $actual_title = $ai_title;
        }
    }

    # Remove any existing title line from content (we'll add the proper one)
    $content =~ s/^#\s+[^\n]+\n*//;
    $content =~ s/^\s+//;

    # Ensure title is first line
    $content = "# $actual_title\n\n$content";

    return {
        id => generate_uuid(),
        createdAt => $created,
        modifiedAt => $updated,
        content => $content,
        tags => \@tags,
        attachments => \@attachments,
        pinned => JSON::false,
        syntaxLanguage => 'markdown',
        showPreview => JSON::true,
    };
}

# ============================== Utilities ==================================

sub generate_uuid {
    # Use CPAN module if available
    if ($HAS_UUID_MODULE eq 'Data::UUID') {
        my $ug = Data::UUID->new();
        return lc($ug->create_str());
    }

    if ($HAS_UUID_MODULE eq 'UUID::Tiny') {
        # UUID_V4 = 4
        return lc(UUID::Tiny::create_uuid_as_string(4));
    }

    # Fall back to RFC 4122 compliant UUID v4 using /dev/urandom
    if (open my $fh, '<', '/dev/urandom') {
        binmode $fh;
        my $bytes;
        read $fh, $bytes, 16;
        close $fh;

        # Set version (4) and variant (10) bits per RFC 4122
        my @b = unpack('C16', $bytes);
        $b[6] = ($b[6] & 0x0f) | 0x40;  # Version 4
        $b[8] = ($b[8] & 0x3f) | 0x80;  # Variant 10

        return sprintf('%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x',
            @b[0..15]);
    }

    # Last resort: Perl rand() based UUID v4 (less secure but portable)
    my @bytes = map { int(rand(256)) } 1..16;
    $bytes[6] = ($bytes[6] & 0x0f) | 0x40;  # Version 4
    $bytes[8] = ($bytes[8] & 0x3f) | 0x80;  # Variant 10

    return sprintf('%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x',
        @bytes[0..15]);
}

sub sanitize_tag {
    my ($tag) = @_;
    $tag = lc($tag);
    $tag =~ s/[^a-z0-9-_]+/-/g;
    $tag =~ s/-+/-/g;
    $tag =~ s/^-|-$//g;
    return $tag;
}

sub guess_extension {
    my ($mime) = @_;

    my %mime_map = (
        # Images
        'image/jpeg' => 'jpg',
        'image/jpg' => 'jpg',
        'image/png' => 'png',
        'image/gif' => 'gif',
        'image/svg+xml' => 'svg',
        'image/webp' => 'webp',
        'image/bmp' => 'bmp',
        'image/tiff' => 'tiff',
        'image/x-icon' => 'ico',
        # Documents
        'application/pdf' => 'pdf',
        'application/msword' => 'doc',
        'application/vnd.openxmlformats-officedocument.wordprocessingml.document' => 'docx',
        'application/vnd.ms-excel' => 'xls',
        'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' => 'xlsx',
        'application/vnd.ms-powerpoint' => 'ppt',
        'application/vnd.openxmlformats-officedocument.presentationml.presentation' => 'pptx',
        # Text
        'text/plain' => 'txt',
        'text/html' => 'html',
        'text/css' => 'css',
        'text/javascript' => 'js',
        'application/json' => 'json',
        'application/xml' => 'xml',
        'text/xml' => 'xml',
        # Audio/Video
        'audio/mpeg' => 'mp3',
        'audio/wav' => 'wav',
        'audio/ogg' => 'ogg',
        'video/mp4' => 'mp4',
        'video/webm' => 'webm',
        'video/quicktime' => 'mov',
        # Archives
        'application/zip' => 'zip',
        'application/x-rar-compressed' => 'rar',
        'application/x-7z-compressed' => '7z',
        'application/gzip' => 'gz',
        # Other
        'application/octet-stream' => 'bin',
    );

    return $mime_map{$mime} || 'bin';
}

# ================================= Main ====================================

sub main {
    my @all_notes;
    my $total_files = scalar @input_files;

    log_msg("Processing $total_files notebook(s)");

    for my $input_file (@input_files) {
        unless (-f $input_file) {
            log_msg("Skipping non-existent file: $input_file");
            next;
        }

        my $notebook_tag = sanitize_tag(basename($input_file, '.enex'));
        log_msg("Processing notebook: $input_file (tag: $notebook_tag)");

        my @note_nodes = parse_enex($input_file);
        my $total = scalar @note_nodes;

        my $index = 0;
        for my $note_node (@note_nodes) {
            $index++;
            my $note = process_note($note_node, $index, $total, $notebook_tag);
            push @all_notes, $note if $note;
        }
    }

    # Determine output file
    my $output;
    if ($opts{'output'}) {
        $output = $opts{'output'};
    } elsif ($total_files == 1) {
        $output = $input_files[0];
        $output =~ s/\.enex$/.json/i;
    } else {
        $output = 'combined.json';
    }

    # Write output - ensure proper UTF-8 encoding
    my $json = JSON->new->utf8->pretty->canonical;
    my $data = {
        version => '1.0',
        exportDate => gmtime->datetime . 'Z',
        notes => \@all_notes,
    };

    open my $fh, '>:raw', $output or die "Cannot write to $output: $!\n";
    print $fh $json->encode($data);
    close $fh;

    log_msg("✅ Wrote " . scalar(@all_notes) . " notes from $total_files notebook(s) to $output");
}

main();
