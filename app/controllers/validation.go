package controllers

import (
	"log"

	"github.com/gin-gonic/gin"
	"github.com/jackc/pgx/v5"
	"github.com/mickaelyoshua/finances/models"
	"github.com/mickaelyoshua/finances/util"
)

func validateRegisterParams(server *models.Server, ctx *gin.Context, name, email, password, confirmPassword string) (map[string]string, error) {
	errs := make(map[string]string, 4)

	if len(name) == 0 {
		errs["name"] = "Name is required"
	} else if len(name) < 3 {
		errs["name"] = "Name must be at least 3 characters long"
	} else if len(name) > 50 {
		errs["name"] = "Name must be at most 50 characters long"
	}

	if len(email) == 0 {
		errs["email"] = "Email is required."
	} else if !util.ValidEmail(email) {
		errs["email"] = "Please provide a valid email address"
	}

	if len(password) == 0 {
		errs["password"] = "Password is required"
	} else if len(password) < 6 {
		errs["password"] = "Password must be at least 6 characters long"
	}

	if len(confirmPassword) == 0 {
		errs["confirmPassword"] = "Password confirmation is required"
	} else if password != confirmPassword {
		errs["confirmPassword"] = "Passwords do not match"
	}

	// If there are already validation errors, no need to check the database.
	if len(errs) > 0 {
		return errs, nil
	}

	_, err := server.Querier.SearchEmail(ctx, email)
	if err == nil {
		errs["email"] = "Email already taken"
		return errs, nil
	}

	if err != pgx.ErrNoRows {
		log.Printf("Database error while searching for email: %v", err)
		return nil, err
	}

	// If get here it got a pgx.ErrNoRows, the given email is not taken
	return errs, nil
}


func validateLoginParams(server *models.Server, ctx *gin.Context, email, password string) (map[string]string, error) {
	errs := make(map[string]string, 3)

	if len(email) == 0 {
		errs["email"] = "Email is required"
	} else if !util.ValidEmail(email) {
		errs["email"] = "Please provide a valid email address"
	}

	if len(password) == 0 {
		errs["password"] = "Password is required"
	} else if len(password) < 6 {
		errs["password"] = "Password must be at least 6 characters long"
	}

	// If there are already validation errors, no need to check the database.
	if len(errs) > 0 {
		return errs, nil
	}

	user, err := server.Querier.GetUserByEmail(ctx, email)
	if err != nil {
		if err == pgx.ErrNoRows {
			errs["login"] = "Email or password incorrect"
			return errs, nil
		}

		log.Printf("Database error while getting user by email: %v", err)
		return nil, err
	}

	// User was found, now check password.
	if !util.PassEqual(user.PasswordHash, password) {
		errs["login"] = "Email or password incorrect"
	}

	return errs, nil
}
