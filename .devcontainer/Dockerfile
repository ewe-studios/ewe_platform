ARG VARIANT=1-bullseye
FROM mcr.microsoft.com/devcontainers/rust:${VARIANT}

ARG USERNAME=ewestudio
ARG USER_UID=1001
ARG USER_GID=${USER_UID}

COPY .vscode/git.alias /home/${USERNAME}/.git.alias
COPY .vscode/.gitconfig /home/${USERNAME}/.gitconfig

RUN	echo 'source $HOME/.git.alias' >> /home/${USERNAME}/.bashrc \
    && echo ${USER_GID} \
    && echo ${USER_UID} \
    && groupadd --gid ${USER_GID} ${USERNAME} || true \
    && useradd --uid ${USER_UID} --gid ${USER_GID} --shell /bin/bash ${USERNAME} \
    && apt-get update \
    && apt-get install -y sudo \
    && echo ${USERNAME} ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/${USERNAME} \
    && chmod 0440 /etc/sudoers.d/${USERNAME} \
    && chown -R ${USERNAME}:${USER_GID} /home/${USERNAME}

USER ${USERNAME}

VOLUME /home/${USERNAME}/.vscode-server