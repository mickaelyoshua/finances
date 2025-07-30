package main

import (
	"context"
	"log"

	"github.com/gin-gonic/gin"
	"github.com/mickaelyoshua/finances/controllers"
	"github.com/mickaelyoshua/finances/db"
	"github.com/mickaelyoshua/finances/db/sqlc"
	"github.com/mickaelyoshua/finances/models"
	"github.com/mickaelyoshua/finances/util"
)

func main() {
	// Get env variables
	config, err := util.LoadConfig(".")
	if err != nil {
		log.Printf("Error loading config: %v\n", err)
		return
	}

	ctx := context.Background()
	conn, err := db.NewConn(config.GetConnString())
	if err != nil {
		log.Printf("Error connecting to database: %v\n", err)
		return
	}
	defer conn.Close(ctx)

	router := gin.Default()

	querier := sqlc.New(conn)
	server := models.NewServer(router, querier)

	// Run server
	SetupRoutes(server)
	server.Router.Run(":"+config.ServerPort)
}

func SetupRoutes(server *models.Server) {
	server.Router.Static("/public", "./public")
	server.Router.GET("/", controllers.Index)

	// Authentication
	server.Router.GET("/register", controllers.RegisterView)
	server.Router.POST("/register", controllers.Register(server))

	server.Router.GET("/login", controllers.LoginView)
}
