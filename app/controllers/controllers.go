package controllers

import (
	"log"
	"net/http"

	"github.com/a-h/templ"
	"github.com/gin-gonic/gin"
	"github.com/google/uuid"
	"github.com/mickaelyoshua/finances/db/sqlc"
	"github.com/mickaelyoshua/finances/models"
	"github.com/mickaelyoshua/finances/util"
	"github.com/mickaelyoshua/finances/views"
)

// Render Templ template
func Render(ctx *gin.Context, status int, template templ.Component) error {
	ctx.Status(status)
	return template.Render(ctx.Request.Context(), ctx.Writer)
}
func HandleRenderError(err error) {
	if err != nil {
		log.Printf("Error rendering template: %v\n", err)
	}
}

func Index(server *models.Server) gin.HandlerFunc {
	return func(ctx *gin.Context) {
		idStr, err := ctx.Cookie("id")
		if err != nil {
			log.Printf("Error getting cookie: %v\n", err)
			return
		}
		id, err := uuid.FromBytes([]byte(idStr))
		if err != nil {
			log.Printf("Error getting uuid from string: %v\n", err)
			return
		}

		user, err := server.Querier.GetUser(ctx, id)
		if err != nil {
			log.Printf("Error getting user: %v\n", err)
			return
		}

		u := sqlc.CreateUserRow{
			ID: user.ID,
			Username: user.Username,
			Email: user.Email,
			CreatedAt: user.CreatedAt,
			UpdatedAt: user.UpdatedAt,
		}
		err = Render(ctx, http.StatusOK, views.Index(u))
		HandleRenderError(err)
	}
}

								//Authentication
func RegisterView(ctx *gin.Context) {
	err := Render(ctx, http.StatusOK, views.Register())
	HandleRenderError(err)
}

func validateRegisterParams(username, email, password, confirmPassword string) map[string]string {
	errors := make(map[string]string)

	if len(username) == 0 {
		errors["username"] = "Username is required."
	} else if len(username) < 3 {
		errors["username"] = "Username must be at least 3 characters long."
	}

	if len(email) == 0 {
		errors["email"] = "Email is required."
	} else if !util.ValidEmail(email) {
		errors["email"] = "Please provide a valid email address."
	}

	if len(password) == 0 {
		errors["password"] = "Password is required."
	} else if len(password) < 6 {
		errors["password"] = "Password must be at least 6 characters long."
	}

	if len(confirmPassword) == 0 {
		errors["confirmPassword"] = "Password confirmation is required."
	} else if password != confirmPassword {
		errors["confirmPassword"] = "Passwords do not match."
	}

	return errors
}
func Register(server *models.Server) gin.HandlerFunc {
	return func(ctx *gin.Context) {
		// Get form params
		username := ctx.PostForm("username")
		email := ctx.PostForm("email")
		password := ctx.PostForm("password")
		confirmPassword := ctx.PostForm("confirm-password")

		// Validate params
		errors := validateRegisterParams(username, email, password, confirmPassword)
		if errors["username"] != "" || errors["email"] != "" || errors["password"] != "" || errors["confirmPassword"] != "" {
			formData := views.RegisterFormData{
				Values: map[string]string{
					"username": username,
					"email":    email,
				},
				Errors: errors,
			}
			err := Render(ctx, http.StatusBadRequest, views.RegisterForm(formData))
			HandleRenderError(err)
			return
		}

		// Hash password
		hashedPass, err := util.HashPassword(password)
		if err != nil {
			ctx.String(http.StatusInternalServerError, "Error hashing password %v", err)
			return
		}

		// Create user
		userParams := sqlc.CreateUserParams{
			Username: username,
			Email: email,
			PasswordHash: hashedPass,
		}
		_, err = server.Querier.CreateUser(ctx, userParams)
		if err != nil {
			ctx.String(http.StatusInternalServerError, "Error creating user %v", err)
			return
		}

		// Redirect
		ctx.Header("HX-Redirect", "/login")
		ctx.Status(http.StatusNoContent)
	}
}

func LoginView(ctx *gin.Context) {
	err := Render(ctx, http.StatusOK, views.Login())
	HandleRenderError(err)
}

