.PHONY: all

all:
	cd QPG-Application && \
	npm install && \
	npm run tauri build && \
	./src-tauri/target/release/qpg-application
