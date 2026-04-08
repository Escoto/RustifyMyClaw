.PHONY: push squash pull

push:
ifndef m
	$(error Usage: make push m="commit message")
endif
	git add .
	git commit -m "$(m)"
	git push

squash:
ifndef m
	$(error Usage: make squash m="commit message")
endif
	git reset --soft $$(git merge-base HEAD main)
	git commit -m "$(m)"
	git push --force

pull:
	git fetch
	git pull