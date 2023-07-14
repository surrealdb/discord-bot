FROM ubuntu:22.04

WORKDIR /app
COPY . .

CMD [ "target/release/surreal_bot" ]