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

		user, err := server.Querier.GetUserById(ctx, id)
		if err != nil {
			log.Printf("Error getting user: %v\n", err)
			return
		}

		u := sqlc.GetUserByIdRow{
			ID: user.ID,
			Name: user.Name,
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

func Register(server *models.Server) gin.HandlerFunc {
	return func(ctx *gin.Context) {
		name := ctx.PostForm("name")
		email := ctx.PostForm("email")
		password := ctx.PostForm("password")
		confirmPassword := ctx.PostForm("confirm-password")

		// Validate params
		errors, err := validateRegisterParams(server, ctx, name, email, password, confirmPassword)
		if err != nil {
			ctx.String(http.StatusInternalServerError, "Error validating Form inputs %v", err)
			return
		}
		if errors["name"] != "" || errors["email"] != "" || errors["password"] != "" || errors["confirmPassword"] != "" {
			formData := views.RegisterFormData{
				Values: map[string]string{
					"name": name,
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
			Name: name,
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

func Login(server *models.Server) gin.HandlerFunc {
	return func(ctx *gin.Context) {
		// Get form params
		email := ctx.PostForm("email")
		password := ctx.PostForm("password")

		// Search user

		// Validate params
		errors, err := validateLoginParams(server, ctx, email, password)
		if err != nil {
			ctx.String(http.StatusInternalServerError, "Error validating Form inputs %v", err)
			return
		}
		if errors["email"] != "" || errors["password"] != "" || errors["login"] != "" {
			formData := views.LoginFormData{
				Values: map[string]string{
					"email":    email,
				},
				Errors: errors,
			}
			err := Render(ctx, http.StatusBadRequest, views.LoginForm(formData))
			HandleRenderError(err)
			return
		}


	}
}
