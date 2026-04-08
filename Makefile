.PHONY: push

push:
ifndef m
	$(error Usage: make push m="commit message")
endif
	git add .
	git commit -m "$(m)"
	git push
