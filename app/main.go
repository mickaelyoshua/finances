package main

import (
	"log"

	"github.com/gin-gonic/gin"
	"github.com/mickaelyoshua/finances/db"
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

	// Get DB connection
	conn, err := db.NewConn(config.GetConnString())
	if err != nil {
		log.Printf("Error connecting to database: %v\n", err)
		return
	}

	// Run server
	router := gin.Default()
	
	server := models.NewServer(router, conn)
	server.Router.Run()
}
