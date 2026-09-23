# GitHub Actions CI/CD 自动发版

**文件位置**: `.github/workflows/release.yml`（已创建但需完善）

## 工作流程

### 1. CI 测试（每次 push）

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: 安装 Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1
        
      - name: 运行后端测试
        run: cargo test --manifest-path rs/Cargo.toml
        
      - name: 安装 Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          
      - name: 安装前端依赖
        run: npm ci --prefix ui
        
      - name: 前端 lint
        run: npm run lint --prefix ui
        
      - name: 前端构建
        run: npm run build --prefix ui
```

### 2. 自动发版（打 tag 时）

```yaml
name: Release
on:
  push:
    tags:
      - 'v*'

jobs:
  build-tauri-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: 安装 Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1
        
      - name: 安装 Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          
      - name: 安装前端依赖
        run: npm ci --prefix ui
        
      - name: 构建 Tauri App
        run: |
          cd ui
          npm run tauri build
          
      - name: 上传 Release Asset
        uses: actions/upload-artifact@v4
        with:
          name: jev-switch-windows
          path: ui/src-tauri/target/release/bundle/msi/*.msi
          
  build-docker:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: 登录 GitHub Container Registry
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
          
      - name: 构建并推送 Docker 镜像
        uses: docker/build-push-action@v5
        with:
          context: .
          push: true
          tags: |
            ghcr.io/${{ github.repository }}:${{ github.ref_name }}
            ghcr.io/${{ github.repository }}:latest
```

## 触发方式

```bash
# 创建新版本 tag
git tag v0.6.0 -m "Release v0.6.0"
git push origin v0.6.0
```

## 产物

- **Windows Tauri APP**: `.msi` 安装包
- **Docker 镜像**: `ghcr.io/arcj137442/jev-switch:v0.6.0`
