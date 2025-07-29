MIGRATION_DIR=./app/db/migration/

PHONY: run-pgadmin stop-pgadmin run-app migrate-up migrate-down

run-pgadmin:
	docker-compose up -d pgadmin

stop-pgadmin:
	docker-compose stop pgadmin

run-app:
	air -c app/.air.toml

migrate-create:
	migrate create -ext sql -dir ${MIGRATION_DIR} -seq schema

migrate-up:


migrate-down:
