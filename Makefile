MIGRATION_DIR=./app/db/migration/
DB_URL=${FINANCES_DATABASE_URL}

PHONY: run-pgadmin stop-pgadmin run-app migrate-create migrate-up migrate-down

run-pgadmin:
	docker-compose up -d pgadmin

stop-pgadmin:
	docker-compose stop pgadmin

run-app:
	cd app && air

migrate-create:
	migrate create -ext sql -dir ${MIGRATION_DIR} -seq schema

migrate-up:
	migrate -database ${DB_URL} -path ./app/db/migration up

migrate-down:
	migrate -database ${DB_URL} -path ./app/db/migration down
