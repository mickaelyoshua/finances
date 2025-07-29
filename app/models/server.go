package models

import (
	"github.com/gin-gonic/gin"
	"github.com/jackc/pgx/v5"
	"github.com/mickaelyoshua/finances/controllers"
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

func (server *Server) SetupRoutes() {
	server.Router.GET("/", controllers.Index)
}
