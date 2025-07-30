#!/bin/bash

# Set the destination folder
DEST_FOLDER="app/public"

# Create the target directory, creating parent directories if needed
mkdir -p "$DEST_FOLDER/scripts"

# Download htmx core library
curl -o "$DEST_FOLDER/scripts/htmx.min.js" https://cdn.jsdelivr.net/npm/htmx.org@2.0.6/dist/htmx.min.js

# Download the response-targets extension
curl -o "$DEST_FOLDER/scripts/response-targets.js" https://cdn.jsdelivr.net/npm/htmx.org@2.0.6/dist/ext/response-targets.js

echo "HTMX files downloaded successfully to $DEST_FOLDER/scripts/"
