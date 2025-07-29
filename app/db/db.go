package db

import (
	"context"

	"github.com/jackc/pgx/v5"
)

func NewConn(connString string)(*pgx.Conn, error) {
	return pgx.Connect(context.Background(), connString)
}
