package main

import (
	"log"

	"github.com/gin-gonic/gin"
	"github.com/mickaelyoshua/finances/db"
	"github.com/mickaelyoshua/finances/models"
)

func Test() {

}

func main() {
	router := gin.Default()

	conn, err := db.NewConn()
	if err != nil {
		log.Fatalf("Error connection with database: %w\n", err)
		return
	}
	server := models.NewServer(router, conn)
	server.Router.Run()
}
