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
			log.Printf("Error getting cookie: %v\n\n", err)
			return
		}
		id, err := uuid.FromBytes([]byte(idStr))
		if err != nil {
			log.Printf("Error getting uuid from string: %v\n\n", err)
			return
		}

		user, err := server.Querier.GetUserById(ctx, id)
		if err != nil {
			log.Printf("Error getting user: %v\n\n", err)
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
		// Get form params
		name := ctx.PostForm("name")
		email := ctx.PostForm("email")
		password := ctx.PostForm("password")
		confirmPassword := ctx.PostForm("confirm-password")

		// Validate params
		validationErrors, err := validateRegisterParams(server, ctx, name, email, password, confirmPassword)
		if err != nil {
			log.Printf("Error validating register params: %v\n\n", err)
			err := Render(ctx, http.StatusInternalServerError, views.FiveHundred())
			HandleRenderError(err)
			return
		}
		if validationErrors != nil {
			formData := views.RegisterFormData{
				Values: map[string]string{
					"name": name,
					"email":    email,
				},
				Errors: validationErrors,
			}
			err := Render(ctx, http.StatusBadRequest, views.RegisterForm(formData))
			HandleRenderError(err)
			return
		}

		// Hash password
		hashedPass, err := util.HashPassword(password)
		if err != nil {
			log.Printf("Error hashing password: %v\n\n", err)
			err := Render(ctx, http.StatusInternalServerError, views.FiveHundred())
			HandleRenderError(err)
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
			log.Printf("Database error creating user: %v\n\n", err)
			err := Render(ctx, http.StatusInternalServerError, views.FiveHundred())
			HandleRenderError(err)
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

		// Validate params
		user, validationErrors, err := validateLoginParams(server, ctx, email, password)
		if err != nil {
			log.Printf("Error validating login params: %v\n\n", err)
			err := Render(ctx, http.StatusInternalServerError, views.FiveHundred())
			HandleRenderError(err)
			return
		}
		if validationErrors != nil {
			formData := views.LoginFormData{
				Email: email,
				Errors: validationErrors,
			}
			err := Render(ctx, http.StatusBadRequest, views.LoginForm(formData))
			HandleRenderError(err)
			return
		}

		// Set cookie
		cookie := http.Cookie{
			Name: "id",
			Value: user.ID.String(),
			MaxAge: 3600,
			HttpOnly: true,
			SameSite: http.SameSiteLaxMode,
		}
		http.SetCookie(ctx.Writer, &cookie)

		// Redirect
		ctx.Header("HX-Redirect", "/")
		ctx.Status(http.StatusNoContent)
	}
}
