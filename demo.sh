#!/bin/bash
set -euo pipefail
IFS=$'\n\t'

#### build

(
    cd PRay
    cmake .
    cd client
    make pray_client
)

cargo build --release

#### run

ADDRESS=127.0.0.1
PORT=1234
CLIENTS_COUNT=6

for (( c=1; c<=CLIENTS_COUNT; c++ ))
do
    (
        cd PRay/client
        ./pray_client --server=$ADDRESS --port=$PORT > /dev/null 2>&1 &
    )
done

rm -f image*.png

cargo run --release -- -s '../scenes/testScene1.xml' -w 1920 -y 1080 -a $ADDRESS -p $PORT -c ${CLIENTS_COUNT} -v
