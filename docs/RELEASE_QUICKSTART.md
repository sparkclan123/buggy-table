# รัน buggy-table และปล่อยรุ่นทดสอบ

รุ่น `v0.1.4` ใช้ไฟล์จาก GitHub Actions run `38496464150` (source commit `b46a10861a4225ddf474daa98c9df99e30f9124e`) เป็น **Pre-release สำหรับทดสอบ** ไม่มี Apple Developer ID/notarization หรือ Windows Authenticode certificate และไม่มี updater ในรุ่นนี้

ดาวน์โหลดที่ https://github.com/sparkclan123/buggy-table-releases/releases/tag/v0.1.4

| เครื่อง | ไฟล์ |
|---|---|
| Mac ชิป M1/M2/M3/M4 และ Apple Silicon รุ่นอื่น | `buggy-table_0.1.4_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.4_x64.dmg` |
| Windows x64 | `buggy-table_0.1.4_x64-setup.exe` |

ผู้ใช้ทั่วไปดาวน์โหลดตัวติดตั้งได้เลย ไม่ต้องลง Rust, Node.js หรือรันคำสั่ง build ระบบอาจเตือนหรือบล็อกไฟล์เพราะยังไม่มีใบรับรองผู้เผยแพร่ หากเครื่ององค์กรห้ามแอปที่ยังไม่เซ็น ต้องให้ผู้ดูแลตรวจสอบก่อน รุ่นนี้ไม่ใช่ signed production release

## เปิดแอปจากซอร์สบน Mac เครื่องนี้

เปิด **Terminal** แล้วรันในโฟลเดอร์โปรเจกต์:

```sh
cd /Users/por/Documents/project/table_por
source "$HOME/.nvm/nvm.sh"
nvm use
source "$HOME/.cargo/env"
pnpm install --frozen-lockfile
pnpm desktop
```

## สร้างตัวติดตั้ง Mac เอง

ใน Terminal เดิมและโฟลเดอร์เดิม:

```sh
pnpm desktop:build --bundles app,dmg
open src-tauri/target/release/bundle/dmg
```

เครื่อง Apple Silicon จะได้ตัวติดตั้ง ARM ส่วน Intel/Windows ให้ build บน GitHub Actions ตามหัวข้อถัดไป คำสั่งนี้ไม่เปิด updater และไม่ใช้ signing Secrets

## ปล่อยรุ่นทดสอบถัดไป เช่น 0.1.5

ใช้ Terminal บน Mac เครื่องนี้ ไม่ต้องรันบน Windows (workflow อยู่ใน branch `main` ):

```sh
cd /Users/por/Documents/project/table_por
source "$HOME/.nvm/nvm.sh"
nvm use
git switch main
git pull --ff-only
pnpm release:version 0.1.5
```

แก้ `docs/RELEASE_NOTES.md` ให้ตรงรุ่น แล้วตรวจและ commit:

```sh
pnpm release:check
pnpm check
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock docs/RELEASE_NOTES.md
git commit -m "Prepare 0.1.5 preview"
git push origin main
git tag -a preview/v0.1.5 -m "buggy-table 0.1.5 preview"
git push origin preview/v0.1.5
gh workflow run installers.yml --repo sparkclan123/buggy-table --ref preview/v0.1.5
gh run list --repo sparkclan123/buggy-table --workflow installers.yml --limit 5
```

ใช้ tag `preview/v…` สำหรับไฟล์ที่ไม่เซ็น เพราะ tag `v…` ของ repo ซอร์สจะเรียก workflow ที่ต้องใช้ Apple/Azure Secrets เก็บเลข run ที่เพิ่งสร้างแล้วแทน `RUN_ID` ในคำสั่งถัดไป:

```sh
gh run watch RUN_ID --repo sparkclan123/buggy-table --exit-status
gh run download RUN_ID --repo sparkclan123/buggy-table --dir artifacts/preview-v0.1.5
```

ทำต่อเฉพาะเมื่อ **ทั้งสามแพลตฟอร์มสำเร็จ** แล้วรวบรวมไฟล์และ checksum:

```sh
mkdir -p artifacts/release-v0.1.5
cp artifacts/preview-v0.1.5/TEST-ONLY-mac-arm64/dmg/*.dmg artifacts/release-v0.1.5/
cp artifacts/preview-v0.1.5/TEST-ONLY-mac-intel/dmg/*.dmg artifacts/release-v0.1.5/
cp artifacts/preview-v0.1.5/TEST-ONLY-windows-x64/nsis/*.exe artifacts/release-v0.1.5/
(cd artifacts/release-v0.1.5 && shasum -a 256 *.dmg *.exe > SHA256SUMS.txt)
gh release create v0.1.5 artifacts/release-v0.1.5/* --repo sparkclan123/buggy-table-releases --target main --title "buggy-table 0.1.5 — Preview" --notes-file docs/RELEASE_NOTES.md --prerelease --draft
```

ตรวจไฟล์ใน draft และ release notes ต้องระบุว่า **ไม่มีใบรับรอง OS และไม่มี updater** แล้วจึงเผยแพร่:

```sh
gh release edit v0.1.5 --repo sparkclan123/buggy-table-releases --draft=false --prerelease --latest=false
```

การเผยแพร่ทำให้ทุกคนดาวน์โหลดได้ ไม่แก้ทับไฟล์รุ่นที่เผยแพร่แล้ว หากแก้แอปให้ขึ้นเลขเวอร์ชันใหม่

## เมื่อพร้อมแจกแบบเซ็นและอัปเดตในแอป

ใช้ [DISTRIBUTION.md](DISTRIBUTION.md) ตั้งค่า Apple และเลือกวิธีเซ็น Windows ที่รองรับประเทศของผู้สมัครก่อน จากนั้นใช้เวอร์ชันใหม่ที่ไม่ซ้ำกับ preview ที่เคยเผยแพร่ รุ่นทดสอบนี้ไม่มี updater จึงต้องดาวน์โหลด signed installer ครั้งแรกด้วยตนเอง หลังจากนั้นจึงอัปเดตผ่านแอปได้
