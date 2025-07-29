package main

import (
	"context"
	"fmt"
	"log"

	"github.com/mickaelyoshua/finances/db"
	"github.com/mickaelyoshua/finances/util"
)

func Test(connString string) error {
	ctx := context.Background()
	conn, err := db.NewConn(connString)
	if err != nil {
		return fmt.Errorf("Error connecting to database: %w\n", err)
	}

	result, err := conn.Query(ctx, "SELECT 1;")
	if err != nil {
		return fmt.Errorf("Error getting query: %w\n", err)
	}
	defer result.Close()
	
	var value int
	for result.Next() {
		err := result.Scan(&value)
		if err != nil {
			return fmt.Errorf("Error getting value from result: %w\n", err)
		}
		log.Printf("Result %v", value)
	}

	return nil
}

func main() {
	config, err := util.LoadConfig(".")
	if err != nil {
		log.Printf("Error loading config: %v\n", err)
		return
	}
	// router := gin.Default()
	//
	// server := models.NewServer(router, conn)
	// server.Router.Run()

	err = Test(config.GetConnString())
	if err != nil {
		log.Printf("%v\n", err)
	}
}
