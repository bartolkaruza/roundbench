#!/bin/bash

export $(cat .env | xargs)
case $1 in
  "nodejs")
    echo "You selected Node.js."
    node nodejs/roundbench.js
    ;;
  "rust")
    echo "You selected Rust."
    # Add your command or function here for when 'rust' is the argument.
    ;;
  "zig")
    echo "You selected Zig."
    # Add your command or function here for when 'zig' is the argument.
    ;;
  *)
    echo "Unknown selection: $1. Please select either 'nodejs', 'rust', or 'zig'."
    ;;
esac