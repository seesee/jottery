#!/usr/bin/perl
use strict;
use warnings;
use JSON;

my $file = $ARGV[0] or die "Usage: $0 <json_file>\n";

open my $fh, '<', $file or die "Cannot open $file: $!\n";
my $json_text = do { local $/; <$fh> };
close $fh;

my $json = JSON->new->utf8->pretty->canonical;
my $data = $json->decode($json_text);

foreach my $note (@{$data->{notes}}) {
  my @attachments = @{$note->{attachments}};
  my $content = $note->{content};
  
  my $index = 0;
  $content =~ s/!\[attachment\]\(attachment:[^\)]+\)/
    $index < @attachments
      ? "![attachment](attachment:" . $attachments[$index++]->{filename} . ")"
      : $&
  /ge;
  
  $note->{content} = $content;
}

open $fh, '>', $file or die "Cannot write to $file: $!\n";
print $fh $json->encode($data);
close $fh;

print "Updated $file\n";
