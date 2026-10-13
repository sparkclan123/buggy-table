# ตัวติดตั้งและอัปเดต buggy-table

ชื่อแอป/ตัวติดตั้งใหม่คือ `buggy-table` ส่วน bundle identifier `dev.tablepor.desktop`, library crate และ preference/signing-key paths เดิมยังคงไว้ เพื่อรักษาความต่อเนื่องของการติดตั้งและค่าที่บันทึกไว้

## เริ่มโดยยังไม่มีใบรับรอง

ดู [คำสั่งรันแอปและปล่อยรุ่นทดสอบ](RELEASE_QUICKSTART.md) สำหรับรุ่นฟรีที่ยังไม่เซ็นด้วย Apple/Windows รุ่นนี้เป็น Pre-release, ไม่มี `latest.json` และปิด updater เพื่อไม่ให้ถูกเสนอเป็นอัปเดตของรุ่นที่เซ็นแล้ว

ขณะเตรียมรุ่น 0.1.0 ตั้ง `TAURI_SIGNING_PRIVATE_KEY` ใน GitHub Environment `release` แล้ว แต่ยังไม่มี Apple/Azure credentials หรือ `RELEASES_TOKEN` การอัปโหลดรุ่นทดสอบด้วย GitHub CLI บนเครื่องเจ้าของ repo ใช้บัญชีที่ล็อกอินอยู่ จึงยังไม่ต้องสร้าง `RELEASES_TOKEN`

สำหรับการแจกแบบเซ็น: สมัคร [Apple Developer Program](https://developer.apple.com/programs/enroll/) (US$99/ปี ราคาท้องถิ่นอาจต่างกัน) แล้วสร้าง Developer ID Application certificate และส่งออก `.p12` พร้อม private key สำหรับ CI

**ตรวจประเทศก่อนเลือก Azure:** ตาม [เงื่อนไข Public Trust](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart) ที่ตรวจเมื่อ 14 ตุลาคม 2026 บุคคลทั่วไปต้องอยู่สหรัฐฯ หรือแคนาดา และประเทศไทยไม่อยู่ในรายชื่อประเทศขององค์กรที่รองรับ หากสมัครในไทย ต้องเลือกผู้ให้บริการ code-signing CA ที่รองรับผู้สมัครของคุณ หรือประเมิน Microsoft Store/MSIX แทน Workflow ปัจจุบันเตรียม Azure ไว้ แต่ยังไม่ผ่านการตั้งค่าบัญชีจริง และต้องปรับ integration หากเลือกผู้ให้บริการอื่น อย่าสมัคร Azure เสียเงินก่อนตรวจสิทธิ์นี้

## โครงสร้างที่เตรียมให้

- ซอร์ส: `sparkclan123/buggy-table` (ตรวจล่าสุดเป็น **private**)
- ไฟล์แจก: https://github.com/sparkclan123/buggy-table-releases
- macOS Apple Silicon → `.dmg` / updater `.app.tar.gz`
- macOS Intel → `.dmg` / updater `.app.tar.gz` คนละไฟล์กับ ARM
- Windows x64 → NSIS `.exe` ใช้ทั้งติดตั้งครั้งแรกและอัปเดต
- `latest.json` มีเวอร์ชัน URL และ signature ของทั้งสามแพลตฟอร์ม; `SHA256SUMS.txt` สำหรับตรวจไฟล์เอง

ซอร์สเป็น private และ repo แจกไฟล์เป็น public ผู้ใช้จึงดาวน์โหลดได้โดยไม่ต้องใช้ token แอปไม่มี GitHub token ฝังอยู่ การเปลี่ยน repo เป็น private ภายหลังไม่ลบสำเนาที่ผู้อื่นดาวน์โหลดไปแล้ว

## ประสบการณ์ผู้ใช้

ติดตั้งครั้งแรกจากหน้า Releases หลังจากนั้นแอปตรวจเวอร์ชันตอนเปิดและทุก 6 ชั่วโมง (เฉพาะ signed release build) มีปุ่มเล็ก `Update v…` มุมล่างขวา กดเปิดรายละเอียด แล้ว `Update & restart` เพื่อดาวน์โหลด ตรวจ signature ติดตั้งทับแอปเดิม และเปิดใหม่ ไม่ต้องไปดาวน์โหลดเองซ้ำ การติดตั้งจะปิด session ฐานข้อมูล จึงต้องเชื่อมต่อใหม่หลังเปิดแอป

ข้อมูลแก้ไขค้างอยู่ทุกแท็บ/ผลบันทึกไม่แน่นอน/คำสั่ง DB ที่ยังรันอยู่จะบล็อกการอัปเดต ธีมและตัวเลือกตรวจอัตโนมัติยังคงอยู่ เปลี่ยนได้ที่ Appearance และ Updates → Check automatically ถ้าเลือกปิดจะไม่มี background check แต่ยังตรวจด้วยปุ่มเองได้

นี่เป็น **full-package update**: ดาวน์โหลดไฟล์แอปรุ่นใหม่ทั้งชุด ไม่ใช่ delta เฉพาะส่วนต่าง วิธีนี้ตรงกับ Tauri updater ที่ใช้ ลดความซับซ้อนในการทำ patch/rollback หากภายหลังไฟล์ใหญ่มากค่อยประเมินระบบ delta เพิ่ม ไม่ควรสัญญาว่าโหลดเฉพาะโค้ดที่เปลี่ยนในรุ่นนี้

Update requests ติดต่อ GitHub/CDN จึงมี IP/user-agent ตามปกติ แต่ไม่มี DB data, SQL, password, host หรือชื่อ connection ไม่มี telemetry เพิ่ม Default/development builds ไม่ตรวจอัปเดต

## ตั้งค่า Secrets ครั้งเดียว

เตรียม GitHub Environment ชื่อ `release` ใน repo ซอร์สแล้ว:
https://github.com/sparkclan123/buggy-table/settings/environments

เพิ่ม Environment Secrets เองผ่านหน้า GitHub (ไม่ส่งลงแชต/commit):

| Secret | ค่า |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | เนื้อหาไฟล์ `~/.config/table-por/signing/updater.key` ที่สร้างให้แล้ว |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | เว้นไว้ได้สำหรับ key ที่สร้างครั้งนี้ ซึ่งไม่มี password |
| `APPLE_CERTIFICATE` | Developer ID Application certificate + private key ส่งออกเป็น `.p12` แล้ว base64 |
| `APPLE_CERTIFICATE_PASSWORD` | password ของ `.p12` |
| `APPLE_SIGNING_IDENTITY` | ชื่อเต็ม เช่น `Developer ID Application: … (TEAMID)` |
| `APPLE_ID` | Apple ID ที่ใช้ notarize |
| `APPLE_PASSWORD` | app-specific password ไม่ใช่รหัสผ่านหลัก Apple ID |
| `APPLE_TEAM_ID` | Apple Developer Team ID |
| `AZURE_CLIENT_ID` | App Registration ที่มีสิทธิ์เซ็น certificate profile |
| `AZURE_CLIENT_SECRET` | client secret ของ App Registration |
| `AZURE_TENANT_ID` | tenant ของ Azure |
| `RELEASES_TOKEN` | fine-grained GitHub PAT จำกัดเฉพาะ `buggy-table-releases`, Contents: read/write |

เพิ่ม Environment Variables (ไม่ใช่ Secrets):

| Variable | ตัวอย่าง/ความหมาย |
|---|---|
| `AZURE_SIGNING_ENDPOINT` | endpoint ตาม region ของบัญชี เช่น `https://eus.codesigning.azure.net` |
| `AZURE_SIGNING_ACCOUNT` | ชื่อ Azure Artifact Signing account |
| `AZURE_SIGNING_PROFILE` | ชื่อ certificate profile |

Azure principal ต้องมีบทบาท Artifact Signing Certificate Profile Signer บน profile ที่เลือก ใช้บริการตาม region/account ของคุณ Workflow ใช้ `artifact-signing-cli` เวอร์ชัน 0.11.0 และ Windows runner ที่มี .NET/Azure CLI/Windows SDK

GitHub ไม่รองรับ secret ช่องว่างเป็น placeholder จึงเตรียม environment, ชื่อที่ workflow อ่าน และเอกสารให้ แทนการสร้างค่าปลอม Workflow จะหยุดหากค่าจำเป็นขาดหาย ตั้ง updater private key แล้ว แต่ยังต้องเพิ่ม Apple/Windows signing credentials และ upload token สำหรับ signed workflow

สำรอง updater private key แบบเข้ารหัสใน password manager/offline backup พร้อมเก็บสิทธิ์เข้าถึงจำกัด Public key ที่แอปเชื่อถืออยู่ใน `src-tauri/tauri.release.conf.json` ห้ามสร้างทับหรือเปลี่ยน key เองหลังแจกแอป ไม่เช่นนั้นผู้ใช้เดิมอัปเดตไม่ได้ สคริปต์ `pnpm release:key` ป้องกันการแทน key ที่ไม่ตรงเดิม

## สร้างและปล่อยรุ่น

1. ใส่ Secrets/Variables ข้างต้น จากนั้น commit/push โค้ดและ workflows ที่ตรวจแล้วไป repo ซอร์ส
2. ตั้งเวอร์ชันเดียวกันทั้ง frontend/Tauri/Rust:

   ```sh
   pnpm release:version 0.1.1
   # แก้ docs/RELEASE_NOTES.md ให้ตรงรุ่น
   pnpm check
   pnpm test
   pnpm test:rust
   pnpm test:ui
   pnpm format:check
   pnpm lint:rust
   node --test scripts/release-assets.test.mjs
   ```

3. Commit ทั้ง version files + Cargo.lock + release notes แล้วสร้าง tag:

   ```sh
   git tag v0.1.1
   git push origin v0.1.1
   ```

   ใช้เลขใหม่ที่ยังไม่เคยเผยแพร่ (0.1.0 สงวนให้รุ่นทดสอบแล้ว) Workflow ตรวจ tag เทียบเวอร์ชันก่อน build หรือเลือก Actions → Build signed release → Run workflow แล้วใส่ existing tag

4. Workflow สร้าง/เซ็น/notarize ทั้งสามแพลตฟอร์ม ตรวจ OS signatures รวม manifest และสร้าง **draft release** ใน repo แจกไฟล์ จะไม่แก้ทับ release ที่มีอยู่ ไม่เผยแพร่ partial build ถ้ามีแพลตฟอร์มใดไม่ผ่าน
5. ดาวน์โหลด artifact จาก Actions ทดสอบติดตั้งและ query ด้วยบัญชีทดสอบ ตรวจชื่อ publisher/Gatekeeper และลองอัปเดตจากรุ่นก่อน เมื่อพร้อมเปิด draft ที่ https://github.com/sparkclan123/buggy-table-releases/releases แล้วกด **Publish release** ให้เป็น latest stable release
6. ผู้ใช้เริ่มเห็นปุ่มรุ่นใหม่ในการตรวจครั้งถัดไป หรือกด Check for updates ทันที

ไฟล์ใน draft ยังไม่ให้ผู้ใช้ทั่วไปโหลดได้ การกด Publish คือขั้นที่เริ่มแจ้งผู้ใช้จริง ห้ามเปลี่ยนไฟล์ของรุ่นที่เผยแพร่แล้ว หากมีบั๊กให้ออกรุ่นที่เลขสูงกว่า ไม่ downgrade ผู้ใช้อัตโนมัติ อย่ารันรุ่นเก่าแล้ว publish เป็น latest

## Build ทดสอบโดยยังไม่ใส่ Secrets

- Mac เครื่องนี้: `pnpm desktop:build --bundles app,dmg`
- Windows: `pnpm desktop:build --bundles nsis` บน Windows ที่ติดตั้ง toolchain ตาม DEVELOPMENT.md
- หรือ Actions → **Build test installers** → Run workflow ได้ artifacts ทั้งสามระบบในหน้า run อายุ 7 วัน ไม่มีระบบอัปเดต/OS certificate ใน build นี้ แจกได้ในฐานะ Pre-release ที่ระบุข้อจำกัดชัดเจน ไม่ใช่ signed production release
- ผลลัพธ์ local อยู่ `src-tauri/target/release/bundle/` ส่วน `.app` อยู่ `macos/`, `.dmg` อยู่ `dmg/`, Windows `.exe` อยู่ `nsis/`

## สิ่งที่ต้องตรวจจริงก่อนแจก

Key สำหรับ updater กับ certificate ของ OS ทำหน้าที่คนละอย่าง ต้องมีทั้งสองชั้น การเซ็น Windows ไม่รับประกันว่า SmartScreen จะไม่เตือนรุ่นแรก การใช้ Azure/Apple ต้องทดสอบกับบัญชีและสิทธิ์จริงของคุณ ยังไม่ถือว่า verified เพียงเพราะ workflow ผ่านการตรวจรูปแบบ

ทดสอบสอง signed versions บน Mac ARM, Mac Intel และ Windows: ตรวจ no-update, update available, bad signature, เน็ตหลุด, disk เต็ม/permission ไม่พอ, เปิดหลาย instance, unsaved edits, app restart และ preference retention การกลับไปใช้รุ่นเดิมในกรณีฉุกเฉินให้ดาวน์โหลด installer ที่เคยเซ็นและเผยแพร่ไว้เอง

อ้างอิง: [Tauri updater](https://v2.tauri.app/plugin/updater/), [macOS signing](https://v2.tauri.app/distribute/sign/macos/), [Windows/Azure signing](https://v2.tauri.app/distribute/sign/windows/), [GitHub runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
