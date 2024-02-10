FROM ubuntu:22.04

RUN apt update
RUN apt install -y libpython3-dev sudo curl gcc python3 python3-pip

WORKDIR /root
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y

RUN mkdir hackathon
WORKDIR /root/hackathon

COPY requirements.txt requirements.txt
RUN --mount=type=cache,target=/root/.cache/pip \
    --mount=type=bind,source=requirements.txt,target=requirements.txt \
    python3 -m pip install -r requirements.txt

COPY download-model.py download-model.py
ENV TRANSFORMERS_CACHE=/tmp/transformers_cache
RUN python3 download-model.py

COPY Cargo.lock .
COPY Cargo.toml .
COPY ntpnet .
COPY pyproc .
COPY rust-toolchain.toml .
COPY scripts .

RUN /root/.cargo/bin/cargo build
