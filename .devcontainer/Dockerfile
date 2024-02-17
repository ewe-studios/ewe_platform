ARG VARIANT=1-bullseye

FROM mcr.microsoft.com/devcontainers/rust:${VARIANT}

COPY .vscode/git.alias /home/vscode/.git.alias

RUN	echo 'source $HOME/.git.alias' >> /home/vscode/.bashrc \
    && wget -O /home/vscode/codelldb-aarch64-linux.vsix https://github.com/vadimcn/codelldb/releases/download/v1.10.0/codelldb-aarch64-linux.vsix  \
    && /usr/local/bin/code --install-extension /home/vscode/codelldb-aarch64-linux.vsix \
    && rm -rf /home/vscode/codelldb-aarch64-linux.vsix \
    && curl https://raw.githubusercontent.com/SeaQL/FireDBG.for.Rust/main/install.sh -sSf | sh
