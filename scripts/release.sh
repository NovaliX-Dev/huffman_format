set -e

cargo test --all
cargo build --release -p cli

mkdir release || :
cp ./target/release/cli ./release/hc

shasum -a 256 ./release/hc > ./release/hc.sha256

echo Done
