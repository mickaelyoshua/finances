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

func validateRegisterParams(server *models.Server, ctx *gin.Context, name, email, password, confirmPassword string) (map[string]string, error) {
	errors := make(map[string]string, 4)

	if len(name) == 0 {
		errors["name"] = "Name is required"
	} else if len(name) < 3 {
		errors["name"] = "Name must be at least 3 characters long"
	} else if len(name) > 50 {
		errors["name"] = "Name must be at most 50 characters long"
	}

	result, err := server.Querier.SearchEmail(ctx, email)
	if err != nil {
		return nil, err
	}
	if len(email) == 0 {
		errors["email"] = "Email is required."
	} else if !util.ValidEmail(email) {
		errors["email"] = "Please provide a valid email address"
	} else if result != "" {
		errors["email"] = "Email already taken"
	}

	if len(password) == 0 {
		errors["password"] = "Password is required"
	} else if len(password) < 6 {
		errors["password"] = "Password must be at least 6 characters long"
	}

	if len(confirmPassword) == 0 {
		errors["confirmPassword"] = "Password confirmation is required"
	} else if password != confirmPassword {
		errors["confirmPassword"] = "Passwords do not match"
	}

	return errors, nil
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

func validateLoginParams(server *models.Server, ctx *gin.Context, email, password string) (map[string]string, error) {
	errors := make(map[string]string, 3)

	if len(email) == 0 {
		errors["email"] = "Email is required"
	} else if !util.ValidEmail(email) {
		errors["email"] = "Please provide a valid email address"
	}

	if len(password) == 0 {
		errors["password"] = "Password is required"
	} else if len(password) < 6 {
		errors["password"] = "Password must be at least 6 characters long"
	}

	// If there are already validation errors, no need to check the database.
	if len(errors) > 0 {
		return errors, nil
	}

	user, err := server.Querier.GetUserByEmail(ctx, email)
	if err != nil {
		return nil, err
	}

	if user.Email == "" || !util.PassEqual(user.PasswordHash, password) {
		errors["login"] = "Email or password incorrect"
	}

	return errors, nil
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
