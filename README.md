# 🚀 ESP32 Mender Client in Rust 🦀

A **pure Rust** implementation of the Mender client for the ESP32 platform, designed for secure and reliable over-the-air (OTA) firmware updates. This project combines the performance and safety of Rust with the robustness of [Mender.io](https://mender.io/), bringing modern OTA solutions to embedded IoT devices.

It is build and test on esp32-c6 for now.
![image](https://github.com/user-attachments/assets/b33603c2-42bd-4634-ac8f-c4ab864c722e)

---

## 🔥 Key Features

- 🔒 **Secure OTA Updates** – Seamless integration with Mender for secure, encrypted firmware delivery.  
- ⚡ **Lightweight & Efficient** – Optimized for resource-constrained ESP32 devices.  
- 🔄 **Automatic Rollback** – Fail-safe mechanisms for firmware deployment.  
- 🦀 **Powered by Rust** – Memory safety and performance without a garbage collector.  
- 📦 **Modular Design** – Easy to extend and adapt for various embedded projects.

---

## 📁 Project Structure

---

## ⚙️ Getting Started

### Prerequisites

- 📦 **Rust Toolchain** (`rustup`)  
- 🛠 **ESP-IDF for Rust** (`espup`)  
- 🔌 **ESP32 Development Board**  
- 🌐 **Mender Server Account** (Hosted or Open Source)

---

### 🚀 Installation & Setup

1. **Install Rust for ESP32**  
   ```bash
   rustup install stable
   cargo install espup
   espup install


2. **Compile and build the project**
  ```bash
  cargo build
  cargo run
