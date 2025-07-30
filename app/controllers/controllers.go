package controllers

import (
	"log"
	"net/http"

	"github.com/a-h/templ"
	"github.com/gin-gonic/gin"
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

// Authentication
func RegisterView(ctx *gin.Context) {
	err := Render(ctx, http.StatusOK, views.Register())
	HandleRenderError(err)
}

func validateCreateUser(username, email, password, confirmPassword string) map[string]string {
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
		username := ctx.PostForm("username")
		email := ctx.PostForm("email")
		password := ctx.PostForm("password")
		confirmPassword := ctx.PostForm("confirm-password")

		errors := validateCreateUser(username, email, password, confirmPassword)

		if len(errors) > 0 {
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

		hashedPass, err := util.HashPassword(password)
		if err != nil {
			ctx.String(http.StatusInternalServerError, "Error hashing password %v", err)
			return
		}

		userParams := sqlc.CreateUserParams{
			Username: username,
			Email: email,
			PasswordHash: hashedPass,
		}
		user, err := server.Querier.CreateUser(ctx, userParams)
		if err != nil {
			ctx.String(http.StatusInternalServerError, "Error creating user %v", err)
			return
		}

		err = Render(ctx, http.StatusOK, views.Index())
	}
}

func LoginView(ctx *gin.Context) {
	err := Render(ctx, http.StatusOK, views.Login())
	HandleRenderError(err)
}

func Index(ctx *gin.Context) {
	err := Render(ctx, http.StatusOK, views.Index())
	HandleRenderError(err)
}
