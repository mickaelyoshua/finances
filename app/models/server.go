package models

import (
	"github.com/gin-gonic/gin"
	"github.com/mickaelyoshua/finances/db/sqlc"
)

type Server struct {
	Router *gin.Engine
	Querier *sqlc.Queries
}

func NewServer(router *gin.Engine, querier *sqlc.Queries) *Server {
	return &Server{
		Router: router,
		Querier: querier,
	}
}
