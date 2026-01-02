#!/usr/bin/perl
use strict;
use warnings;
use LWP::UserAgent;
use HTTP::Request;
use JSON;

my $url = $ARGV[0] or die "Usage: $0 <url>\n";
my $api_key = $ENV{OPENAI_API_KEY} or die
  "OPENAI_API_KEY env var required\n";
my $api_base = $ENV{OPENAI_API_BASE} or die
  "OPENAI_API_BASE env var required\n";
my $model = $ENV{OPENAI_MODEL} || "gpt-4o-mini";


my $lfile = "/tmp/out.log";
open my $fh, '>>', $lfile or die "Cannot write to $lfile: $!\n";

my $ua = LWP::UserAgent->new;
$ua->timeout(10);

my $response = $ua->get($url);
unless ($response->is_success) {
  print STDERR "Failed to fetch $url\n";
  exit 1;
}

my $real_url = $response->request->uri->as_string;
my $content = $response->decoded_content;
$content =~ s/<[^>]+>//g;
$content = substr($content, 0, 2000);

my $json = JSON->new->utf8;
my $request = {
  model => $model,
  messages => [
    {
      role => "user",
      content =>
        "Summarize this in 1-2 paragraphs and suggest up to 3 tags (tags must be lower case and the only non-alpha character allowed is a hyphen). If it's a github project don't comment on css, look for the actual content. If it's not possible to summarise meaningfully, just say that:\n\n" .
        "$content\n\n" .
        "Respond in JSON format with 'summary' and 'tags' keys."
    }
  ]
};

my $req = HTTP::Request->new(
  POST => "$api_base/chat/completions"
);
$req->header('Content-Type' => 'application/json');
$req->header('Authorization' => "Bearer $api_key");
$req->content($json->encode($request));

my $result = $ua->request($req);
if ($result->is_success) {
  my $data = $json->decode($result->decoded_content);
  my $ai_response = $data->{choices}[0]{message}{content};
  
  my $ai_json = $json->decode($ai_response);
  my $output = {
    url => $real_url,
    summary => $ai_json->{summary},
    tags => $ai_json->{tags}
  };
  
  print $json->encode($output);
  print $fh $json->encode($output) . "\n\n";
} else {
  print STDERR "API error: " . $result->status_line . "\n";
  exit 1;
}

close $fh;
