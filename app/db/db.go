package db

import (
	"context"

	"github.com/jackc/pgx/v5"
	"github.com/mickaelyoshua/finances/util"
)

func NewConn()(*pgx.Conn, error) {
	config, err := util.LoadConfig(".")
	if err != nil {
		return nil, err
	}
	connString := config.GetConnString()
	return pgx.Connect(context.Background(), connString)
}
