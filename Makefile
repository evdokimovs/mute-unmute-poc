up:
	$(MAKE) -j2 up.backend up.frontend

up.backend:
	cargo run -p mute-unmute-poc-server

up.frontend:
	cd web && npm run start

deps:
	cd web && yarn install