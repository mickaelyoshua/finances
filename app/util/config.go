package util

import "github.com/spf13/viper"

type Config struct {
	DB_Driver string `mapstructure:"DB_DRIVER"`
	DB_Username string `mapstructure:"DB_USERNAME"`
	DB_Password string `mapstructure:"DB_PASSWORD"`
	DB_Host string `mapstructure:"DB_HOST"`
	DB_Port string `mapstructure:"DB_PORT"`
	DB_Name string `mapstructure:"DB_NAME"`
}

func LoadConfig(path string) (Config, error) {
	var config Config

	viper.AddConfigPath(path)
	viper.SetConfigName("app")
	viper.SetConfigType("env")

	viper.AutomaticEnv() // check if env variables match the existing keys

	err := viper.ReadInConfig()
	if err != nil {
		return config, err
	}

	err = viper.Unmarshal(&config)
	return config, err
}

func (config Config) GetConnString() string {
	return  config.DB_Driver + "://" +
			config.DB_Username + ":" +
			config.DB_Password + "@" +
			config.DB_Host + ":" +
			config.DB_Port + "/" +
			config.DB_Name
}
