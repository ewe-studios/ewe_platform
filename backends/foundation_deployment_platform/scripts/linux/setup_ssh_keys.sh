#!/bin/bash
mkdir -p /home/vagrant/.ssh && chmod 700 /home/vagrant/.ssh && touch /home/vagrant/.ssh/authorized_keys && \
chmod 600 /home/vagrant/.ssh/authorized_keys && \
grep -qxF '{{KEY}}' /home/vagrant/.ssh/authorized_keys || echo '{{KEY}}' >> /home/vagrant/.ssh/authorized_keys
