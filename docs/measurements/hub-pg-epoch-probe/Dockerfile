FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends postgresql-17 && rm -rf /var/lib/apt/lists/*
USER postgres
ENV PATH=/usr/lib/postgresql/17/bin:$PATH
