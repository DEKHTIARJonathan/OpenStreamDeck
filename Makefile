.PHONY: setup update

setup:
	curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
	# cargo install cargo-binstall --locked
	cargo binstall --locked --no-confirm prek
	cargo binstall --locked --no-confirm cargo-outdated
	cargo binstall --locked --no-confirm cargo-machete
	cargo binstall --locked --no-confirm cargo-deny
	cargo binstall --locked --no-confirm cargo-audit
	cargo binstall --locked --no-confirm cargo-wizard
	cargo binstall --locked --no-confirm cargo-nextest
	cargo binstall --locked --no-confirm cargo-wix
	cargo binstall --locked --no-confirm tauri-cli

update:
	prek autoupdate
	bun update
	cargo update
	cargo outdated --workspace --depth 1

.PHONY: test test-frontend test-backend

# Run only tests, keeping frontend and backend runners separate and serial.
test:
	$(MAKE) test-frontend
	$(MAKE) test-backend

test-frontend:
	bun run test
	bun run test:e2e

test-backend:
	cargo test --workspace --all-targets --locked
