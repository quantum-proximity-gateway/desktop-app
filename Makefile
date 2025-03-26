.PHONY: all build dev

all: build

build:
	cd QPG-Application && \
	npm install && \
	npm run tauri build && \
	./src-tauri/target/release/qpg-application

dev:
	cd QPG-Application && \
	npm install && \
	npm run tauri dev
