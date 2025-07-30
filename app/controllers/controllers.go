package controllers

import (
	"log"
	"net/http"

	"github.com/a-h/templ"
	"github.com/gin-gonic/gin"
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
func Register(ctx *gin.Context) {
	username := ctx.PostForm("username")
	email := ctx.PostForm("email")
	password := ctx.PostForm("password")
	confirmPassword := ctx.PostForm("confirm-password")

	if password != confirmPassword {
		err := Render(ctx, http.StatusBadRequest, views.RegisterForm())
		HandleRenderError(err)
		return
	}

	hashedPass, err := util.HashPassword(password)
	if err != nil {
		ctx.Status(http.StatusInternalServerError)
		return
	}
	models.NewUser(username, email, hashedPass)
}

func LoginView(ctx *gin.Context) {
	err := Render(ctx, http.StatusOK, views.Login())
	HandleRenderError(err)
}

func Index(ctx *gin.Context) {
	err := Render(ctx, http.StatusOK, views.Index())
	HandleRenderError(err)
}
