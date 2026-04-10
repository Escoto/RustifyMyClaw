.PHONY: build pull push run squash

build:
	rustc --version
	cargo --version
	cargo build --release

pull:
	git fetch
	git pull

push:
ifndef m
	$(error Usage: make push m="commit message")
endif
	git add .
	git commit -m "$(m)"
	git push

run:
	./target/release/rustifymyclaw

squash:
ifndef m
	$(error Usage: make squash m="commit message")
endif
	git reset --soft $$(git merge-base HEAD main)
	git commit -m "$(m)"
	git push --force
