#!/bin/bash
# Install CPAN dependencies for enex2jottery.pl

set -e

echo "Installing CPAN modules for enex2jottery.pl..."
echo

# Required modules
REQUIRED="XML::LibXML HTML::TreeBuilder LWP::UserAgent JSON LWP::Protocol::https"

# Recommended for better UUID generation
RECOMMENDED="UUID::Tiny"

# Check if cpanm is available
if command -v cpanm &> /dev/null; then
    echo "Using cpanm (fast installer)"
    echo
    echo "Installing required modules..."
    cpanm --notest $REQUIRED

    echo
    echo "Installing recommended modules..."
    cpanm --notest $RECOMMENDED || echo "  (UUID::Tiny install failed, will use fallback UUID generation)"
else
    echo "Using cpan (slower, but works everywhere)"
    echo
    echo "Installing required modules..."
    cpan $REQUIRED

    echo
    echo "Installing recommended modules..."
    cpan $RECOMMENDED || echo "  (UUID::Tiny install failed, will use fallback UUID generation)"
fi

echo
echo "✅ Dependencies installed successfully!"
echo
echo "You can now run:"
echo "  ./enex2jottery.pl --help"
