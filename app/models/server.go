package models

import (
	"github.com/gin-gonic/gin"
	"github.com/jackc/pgx/v5"
)

type Server struct {
	Router *gin.Engine
	Conn *pgx.Conn
}

func NewServer(router *gin.Engine, conn *pgx.Conn) *Server {
	return &Server{
		Router: router,
		Conn: conn,
	}
}
