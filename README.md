CorTeX Peripherals - Rust+ZMQ implementation

## Rust installation for HULK (and other non-sudo clusters)

1. Find a way to get rustup.sh to HULK without https capabilities. Traditionally it is:
  ```
  curl -s https://static.rust-lang.org/rustup.sh
  ```

2. Then perform the installation process, and add the local paths to 
  ```
  tcsh% mkdir ~/.local
  tcsh% ./rustup.sh --channel=nightly --prefix=/home/dginev/.local --disable-sudo

  ```