
all: .provider

.provider:
	cd ../gu-provider && cargo build --release
	cp ../target/release/gu-provider ./provider/
	cd provider && docker build .


