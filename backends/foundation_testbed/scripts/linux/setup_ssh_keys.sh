#!/bin/bash
mkdir -p ~/.ssh && chmod 700 ~/.ssh && touch ~/.ssh/authorized_keys && \
chmod 600 ~/.ssh/authorized_keys && \
grep -qxF '{{KEY}}' ~/.ssh/authorized_keys || echo '{{KEY}}' >> ~/.ssh/authorized_keys
