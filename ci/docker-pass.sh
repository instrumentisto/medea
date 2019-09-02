#!/bin/sh

# see https://github.com/docker/docker-credential-helpers/blob/master/ci/before_script_linux.sh

set -e

# init key for pass
# dont forget to change email
gpg --batch --gen-key <<-EOF
%echo Generating a standard key
Key-Type: DSA
Key-Length: 1024
Subkey-Type: ELG-E
Subkey-Length: 1024
Name-Real: Instrumentisto Team
Name-Email: lapa.alex@ex.ua
Expire-Date: 0
# Do a commit here, so that we can later print "done" :-)
%commit
%echo done
EOF

key=$(gpg --no-auto-check-trustdb --list-secret-keys | grep ^sec | cut -d/ -f2 | cut -d" " -f1)
pass init $key