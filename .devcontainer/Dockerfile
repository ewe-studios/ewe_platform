FROM mcr.microsoft.com/devcontainers/rust:1-bullseye

COPY .vscode/git.alias /home/vscode/.git.alias

RUN echo 'source $HOME/.git.alias' >> /home/vscode/.bashrc