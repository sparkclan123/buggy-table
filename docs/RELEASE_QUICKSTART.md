# รัน buggy-table และปล่อยรุ่นทดสอบ

รุ่น `v0.1.9` เพิ่มแถวสลับสีใน Data และ Row details, ลากจัดลำดับแท็บ, scrollbar แท็บบาง 3 px, ปุ่มรีเฟรช schema แบบไอคอน, ไอคอนตารางสีฟ้า และเริ่มต้น 100 แถวต่อหน้า ปุ่ม Updates เปิดหน้าดาวน์โหลด GitHub มี Mac Apple Silicon, Mac Intel และ Windows x64 รุ่นนี้ยังเป็น **Pre-release** ไม่มี Apple Developer ID/notarization, Windows Authenticode และต้องติดตั้งทับเอง

ดาวน์โหลดที่ https://github.com/sparkclan123/buggy-table-releases/releases/tag/v0.1.9

| เครื่อง | ไฟล์ |
|---|---|
| Mac Apple Silicon (M series) | `buggy-table_0.1.9_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.9_x64.dmg` |
| Windows x64 | ยังรอ build จาก GitHub Actions; ใช้ 0.1.8 ระหว่างนี้ |

ไฟล์ Mac 0.1.9 build และตรวจในเครื่องแล้ว ส่วน Windows และ CI ยังรอคิวเพราะ [GitHub Actions ขัดข้อง](https://www.githubstatus.com/incidents/8tzbpcnl75jp) ติดตาม [installer run](https://github.com/sparkclan123/buggy-table/actions/runs/39284299677) และตรวจไฟล์ก่อนเผยแพร่ Windows เพิ่ม ห้ามเปลี่ยนชื่อไฟล์ 0.1.8 เป็น 0.1.9

**รุ่น 0.1.4–0.1.9 ปิด updater ไว้** ผู้ใช้ต้องดาวน์โหลดติดตั้งทับด้วยตนเอง การเผยแพร่ release ไม่สามารถเปิด updater ในแอปที่ติดตั้งไปแล้ว ก่อนติดตั้งให้บันทึกหรือยกเลิกงานค้างและปิดแอป Windows ให้รัน installer และเลือกตำแหน่งติดตั้งเดิม

ผู้ใช้ทั่วไปไม่ต้องลง Rust หรือ Node.js บน Mac ให้เปิด DMG แล้วลาก buggy-table ไป Applications แทนตัวเก่า โดยไม่ลบโฟลเดอร์ข้อมูลแอป ค่าการเชื่อมต่อเดิมยังอยู่

รุ่นนี้ยังไม่ผ่าน Apple notarization: หลังลองเปิดครั้งแรก ถ้า macOS แจ้งว่าไม่สามารถตรวจสอบผู้พัฒนา และคุณเชื่อถือไฟล์ที่ดาวน์โหลดจาก repo นี้ หากระบบมีตัวเลือก ให้ไป **System Settings → Privacy & Security → Open Anyway → Open** ตาม [ขั้นตอน Apple](https://support.apple.com/en-us/102445) เครื่องที่องค์กรดูแลอาจห้ามอนุญาตเอง ไม่ต้องปิด Gatekeeper หรือใช้คำสั่งลบ quarantine

หากมีเฉพาะ Move to Trash ไม่มี Open Anyway แสดงว่าคำแนะนำนี้ใช้กับเครื่องนั้นไม่ได้ รุ่นปัจจุบันยังไม่ผ่านเกณฑ์แจกของ Apple ต้องใช้ Developer ID + notarization เพื่อแก้การแจกอย่างถูกต้อง

หากไฟล์ Preview ยังขึ้น “damaged” ให้หยุดและตรวจ SHA-256 กับ `SHA256SUMS.txt` พร้อมรายงานเวอร์ชัน macOS/ชนิดชิป อย่าใช้ Open Anyway กับไฟล์ที่ตรวจความสมบูรณ์ไม่ผ่าน

การแจกแบบเปิดใช้งานโดยไม่ต้องอนุญาตผู้พัฒนาที่ไม่ผ่านการยืนยันต้องมี Apple Developer ID + notarization ดู [DISTRIBUTION.md](DISTRIBUTION.md)

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
bash scripts/verify-macos-bundle.sh src-tauri/target/release/bundle preview
open src-tauri/target/release/bundle/dmg
```

เครื่อง Apple Silicon จะได้ตัวติดตั้ง ARM ส่วน Intel/Windows ให้ build บน GitHub Actions ตามหัวข้อถัดไป คำสั่งนี้ไม่เปิด updater และไม่ใช้ signing Secrets

## ปล่อยรุ่นทดสอบถัดไป เช่น 0.1.10

ใช้ Terminal บน Mac เครื่องนี้ ไม่ต้องรันบน Windows (workflow อยู่ใน branch `main` ):

```sh
cd /Users/por/Documents/project/table_por
source "$HOME/.nvm/nvm.sh"
nvm use
git switch main
git pull --ff-only
pnpm release:version 0.1.10
```

แก้ `docs/RELEASE_NOTES.md` ให้ตรงรุ่น แล้วตรวจและ commit:

```sh
pnpm release:check
pnpm check
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock docs/RELEASE_NOTES.md
git commit -m "Prepare 0.1.10 preview"
git push origin main
git tag -a preview/v0.1.10 -m "buggy-table 0.1.10 preview"
git push origin preview/v0.1.10
gh workflow run installers.yml --repo sparkclan123/buggy-table --ref preview/v0.1.10
gh run list --repo sparkclan123/buggy-table --workflow installers.yml --limit 5
```

ใช้ tag `preview/v…` สำหรับไฟล์ที่ไม่เซ็น เพราะ tag `v…` ของ repo ซอร์สจะเรียก workflow ที่ต้องใช้ Apple/Azure Secrets เก็บเลข run ที่เพิ่งสร้างแล้วแทน `RUN_ID` ในคำสั่งถัดไป:

```sh
gh run watch RUN_ID --repo sparkclan123/buggy-table --exit-status
gh run download RUN_ID --repo sparkclan123/buggy-table --dir artifacts/preview-v0.1.10
```

ทำต่อเฉพาะเมื่อ **ทั้งสามแพลตฟอร์มสำเร็จ** แล้วรวบรวมไฟล์และ checksum:

```sh
mkdir -p artifacts/release-v0.1.10
cp artifacts/preview-v0.1.10/TEST-ONLY-mac-arm64/dmg/*.dmg artifacts/release-v0.1.10/
cp artifacts/preview-v0.1.10/TEST-ONLY-mac-intel/dmg/*.dmg artifacts/release-v0.1.10/
cp artifacts/preview-v0.1.10/TEST-ONLY-windows-x64/nsis/*.exe artifacts/release-v0.1.10/
(cd artifacts/release-v0.1.10 && shasum -a 256 *.dmg *.exe > SHA256SUMS.txt)
gh release create v0.1.10 artifacts/release-v0.1.10/* --repo sparkclan123/buggy-table-releases --target main --title "buggy-table 0.1.10 — Preview" --notes-file docs/RELEASE_NOTES.md --prerelease --draft
```

ตรวจไฟล์ใน draft และ release notes ต้องระบุว่า **ไม่มีใบรับรอง OS และไม่มี updater** แล้วจึงเผยแพร่:

```sh
gh release edit v0.1.10 --repo sparkclan123/buggy-table-releases --draft=false --prerelease --latest=false
```

การเผยแพร่ทำให้ทุกคนดาวน์โหลดได้ ไม่แก้ทับไฟล์รุ่นที่เผยแพร่แล้ว หากแก้แอปให้ขึ้นเลขเวอร์ชันใหม่

## เมื่อพร้อมแจกแบบเซ็นและอัปเดตในแอป

ใช้ [DISTRIBUTION.md](DISTRIBUTION.md) ตั้งค่า Apple และเลือกวิธีเซ็น Windows ที่รองรับประเทศของผู้สมัครก่อน จากนั้นใช้เวอร์ชันใหม่ที่ไม่ซ้ำกับ preview ที่เคยเผยแพร่ รุ่นทดสอบนี้ไม่มี updater จึงต้องดาวน์โหลด signed installer ครั้งแรกด้วยตนเอง หลังจากนั้นจึงอัปเดตผ่านแอปได้

## หาก GitHub Actions ไม่เริ่มเพราะ Billing

เมื่อ 14 ตุลาคม 2026 run `38498402139` ถูกบล็อกก่อนเริ่มทุก job ด้วยข้อความ recent account payments have failed or spending limit needs to be increased หากเกิดซ้ำ เจ้าของต้องตรวจ https://github.com/settings/billing สถานะที่ตรวจเมื่อ 19 ตุลาคม 2026 พบว่า source repo เป็น public อยู่แล้ว อย่าเปลี่ยน visibility ของ repo เพื่อแก้ปัญหา CI โดยอัตโนมัติ

Mac ARM สามารถ build บนเครื่องนี้ตามด้านบน ส่วน Mac Intel build ข้ามสถาปัตยกรรมได้ด้วย:

```sh
rustup target add x86_64-apple-darwin
pnpm tauri build --target x86_64-apple-darwin --bundles app,dmg
bash scripts/verify-macos-bundle.sh src-tauri/target/x86_64-apple-darwin/release/bundle preview
```

Windows ต้องใช้ Windows build environment; ไม่เปลี่ยนชื่อ installer รุ่นเก่าให้เป็นเลขรุ่นใหม่
