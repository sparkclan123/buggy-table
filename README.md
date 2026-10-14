# buggy-table

A personal, local-first database workspace built with **Rust, Tauri 2, Svelte and TypeScript**.

รุ่นนี้รองรับ **PostgreSQL 15–18, MySQL 8.x และ Redis standalone** โดย PostgreSQL อ่าน/แก้ไขเซลล์ได้ ส่วน MySQL และ Redis เปิดแบบอ่านอย่างเดียว มีหน้าตัวอย่างสำหรับลอง UI โดยไม่ต้องเชื่อมฐานข้อมูล ยังไม่ครบทุกข้อในแผนระยะยาว

## ใช้งานได้แล้ว

- เลือก PostgreSQL / MySQL / Redis ในหน้าต่อฐานข้อมูล พร้อม port เริ่มต้นตามชนิด
- MySQL: ตารางหลายแท็บ, filter/sort, SQL SELECT, EXPLAIN, structure, indexes และ overview; ต้องใช้บัญชี SELECT/SHOW VIEW เท่านั้น
- Redis: SCAN คีย์ตาม pattern, ดูค่า/ชนิด/TTL, อ่าน string/hash/list/set/sorted set และรันคำสั่งอ่านแบบ JSON arguments
- เชื่อมทั้งสามชนิดด้วย verified TLS หรือ plaintext เฉพาะ literal loopback IP; PostgreSQL มีโหมด TLS แบบไม่ตรวจใบรับรองที่ต้องเลือกเอง และรองรับ CA PEM ส่วนตัว
- เปิดหลายตารางเป็นแท็บด้านบน เก็บ filter, page, SQL, scroll และ pending edits แยกกัน
- คลิกเซลล์แล้วพิมพ์ในตารางได้ทันที: Enter จบการแก้ไข, Escape คืนค่าก่อนแก้, Shift+Enter ขึ้นบรรทัดใหม่; Save changes หรือ Cmd/Ctrl+S เพื่อบันทึก และ Discard เพื่อยกเลิก
- ลากขอบหัวคอลัมน์เพื่อปรับความกว้าง ดับเบิลคลิกขอบเพื่อคืนขนาดเดิม หรือโฟกัสขอบแล้วกดลูกศรซ้าย/ขวา
- คลิกหัวคอลัมน์เพื่อ sort ASC/DESC จากฐานข้อมูลก่อนแบ่งหน้า; เก็บความกว้างและการเรียงแยกตามแท็บ
- เพิ่ม/ลบ filter ได้สูงสุด 20 เงื่อนไข เลือก AND/OR แล้ว Apply filters
- Cmd+R บน Mac, Ctrl+R บน Windows หรือ F5 รีเฟรชข้อมูลตาราง โดยไม่รีโหลดทั้งแอป; ถ้ามีค่ารอ Save จะเตือนก่อน
- Save เป็น transaction เดียว ตรวจ Primary key, UPDATE permission และค่าต้นฉบับก่อนบันทึก
- SQL editor ยังอ่านอย่างเดียว; Manual Save แยกจาก query/AI ไม่มี Delete/Truncate endpoint
- UI กระชับ โทนเรียบ ข้อมูลไม่มี status สี, filter ใช้ bound parameters, pagination และ virtualized grid ตามขนาดหน้าจอ
- SQL editor พร้อม syntax highlighting, completion และ `Cmd/Ctrl + Enter`
- SELECT subset, estimated `EXPLAIN`, table structure, index definitions/scan counters
- Database overview จาก PostgreSQL จริง พร้อมเวลาที่เก็บและคำอธิบาย coverage
- Cancel, server/client deadlines, result row/byte caps และการปฏิเสธคำสั่งเขียนใน SQL editor
- Credentials, SQL และ results อยู่ใน memory ของ session ไม่มีการบันทึกลง application storage
- ไม่มี analytics, cloud AI, remote fonts หรือ download โมเดลอัตโนมัติ
- Light / Dark / System ที่ Appearance; จดจำเฉพาะค่า preference
- Signed release มีปุ่ม Updates มุมล่างขวา ตรวจ GitHub เมื่อเปิดแอปและทุก 6 ชั่วโมง ปิด automatic checks ได้ ไม่มีข้อมูล DB/credential ใน request
- วิธีสร้างตัวติดตั้งและออกรุ่นใหม่: [Distribution guide](docs/DISTRIBUTION.md)

เซลล์ Primary key, identity/generated columns และตารางไม่มี Primary key ยังแก้ไม่ได้ Save จำกัด 100 แถว/500 เซลล์ต่อครั้ง หากเกิด conflict จะไม่บันทึกทั้งชุด หากผล COMMIT ไม่แน่นอน จะบล็อกการบันทึกซ้ำจน discard draft และ reload ทั้งนี้ manual UPDATE ทำงานตาม constraints/triggers ของฐานข้อมูลตามปกติ

**ยังไม่เปิดใช้:** MySQL/Redis manual writes, Redis Cluster/Sentinel/streams, MariaDB, local AI, saved connections/keychain, SSH, query history, joins/CTEs/functions/casts, views/RLS/custom types และการรับรองใช้งาน production

## เริ่มรัน

ต้องมี Node.js 22.12+, pnpm 10, Rust ตาม `rust-toolchain.toml` และ native prerequisites ของ Tauri บน OS นั้น บน macOS ใช้ Xcode Command Line Tools; บน Windows ต้องมี Microsoft C++ build tools และ WebView2

```sh
pnpm install --frozen-lockfile
pnpm desktop
```

ถ้า shell ของเครื่องนี้ยังเลือก Node 16 หรือยังไม่เห็น Cargo:

```sh
nvm use
source "$HOME/.cargo/env"
pnpm desktop
```

ดู UI อย่างเดียวใน browser:

```sh
pnpm dev
```

เปิด `http://127.0.0.1:1420` — browser preview จะไม่เชื่อมฐานข้อมูล และไม่จำลองว่า SQL/AI ทำงานจริง

## ทดลองกับฐานข้อมูลที่แยกไว้ให้

เปิด Docker แล้วรัน:

```sh
docker compose up -d --wait
pnpm desktop
```

กด **Connect database** และกรอก:

| ช่อง | ค่า |
|---|---|
| Connection name | Local test |
| Host | `127.0.0.1` |
| Port | `15432` |
| Database | `tablepor_test` |
| Schema | `public` |
| Username | `tablepor_editor` |
| Password | `local-editor-only` |
| Allow manual cell edits | เปิด |
| Transport security | Local only · unencrypted loopback IP |

เปิดตาราง **edit_items** เพื่อทดลองแก้ label, note, amount แล้ว Save และ refresh ดูผลจริง บัญชี fixture นี้มีสิทธิ์ UPDATE เฉพาะคอลัมน์ที่กำหนด ตารางอื่นยังอ่านได้ ถ้าต้องการทดสอบ strict reader ใช้ `tablepor_reader` / `local-reader-only` แล้วปิด Allow manual cell edits

ข้อมูลและรหัสผ่านข้างต้นเป็น **fixture ทดสอบเท่านั้น** ฐานข้อมูลอยู่ใน Docker tmpfs และแยกจากฐานข้อมูลของคุณ หยุด/ลบ environment ทดสอบด้วย `docker compose down`; ข้อมูลตัวอย่างจะสร้างใหม่เมื่อเริ่ม container ใหม่

ลอง SQL:

```sql
SELECT id, customer, total
FROM public.orders
WHERE total > 100
ORDER BY id
LIMIT 20;
```

`DELETE`, `TRUNCATE`, `DROP`, functions, multi-statements และ query ที่ออกนอก approved scope จะถูกปฏิเสธ โหมด strict reader ปฏิเสธ admin/write roles ส่วน manual editing ใช้บัญชีที่คุณกรอกและตรวจ UPDATE แต่ละคอลัมน์

## ทดลอง MySQL และ Redis

ใน Connect database เลือก Database engine ก่อน ค่า port จะเปลี่ยนตามชนิด ส่วน fixture ใช้ port ด้านล่าง ทั้งสองแบบอ่านอย่างเดียวและไม่มีช่อง manual edit

| Fixture | Host / port | Database | Username | Password |
|---|---|---|---|---|
| MySQL 8.4 | `127.0.0.1:13306` | `tablepor_test` | `tablepor_reader` | `local-mysql-reader` |
| Redis 8 | `127.0.0.1:16379` | `0` | `tablepor_reader` | `local-redis-reader` |

เลือก Local only สำหรับ fixture เท่านั้น สำหรับเครื่องอื่นใช้ verified TLS และ certificate ที่ระบบเชื่อถือ; PostgreSQL รองรับ custom CA และโหมด TLS แบบเข้ารหัสโดยไม่ตรวจใบรับรอง ดู [การแก้ปัญหาเชื่อมต่อ](docs/CONNECTIONS.md) MySQL ต้องมี direct SELECT/SHOW VIEW grants ไม่มี roles/สิทธิ์เขียน ส่วน Redis ควรใช้ ACL user จำกัดคำสั่ง ไม่ต้องให้สิทธิ์ admin; แอปบล็อกคำสั่งเขียน/ลบเองด้วย

MySQL SQL ใช้ชื่อแบบ ``SELECT * FROM `tablepor_test`.`items` LIMIT 200``. Redis เปิดหน้าคีย์แยกจากตาราง SQL ใช้ Scan / refresh และ Next scan; คลิกคีย์เพื่อดู preview และ TTL หรือกรอก `["GET", "my-key"]`, `["INFO", "memory"]`, `["HSCAN", "my-hash", "0"]` แล้ว Run command. GET แสดงสูงสุด 64 KiB; collection แสดงชุดแรกและใช้ cursor อ่านต่อได้ คำสั่งต้องเป็น JSON array เพื่อรองรับชื่อคีย์ที่มีช่องว่างอย่างไม่กำกวม

Redis fixture เริ่มว่าง; integration tests สร้างคีย์ `fixture:*` จากข้อมูลสมมติให้อัตโนมัติ

ผลตรวจความปลอดภัยและ advisory ที่ยังเหลืออ่านใน [SECURITY_REVIEW.md](docs/SECURITY_REVIEW.md) — ยังไม่ใช่การรับรองพร้อมใช้ production

## โค้ดอยู่ตรงไหน

```text
src/
  App.svelte                 UI state และการประสานงานกับ backend
  components/                Dialog, editor, grid และหน้าจอแยกส่วน
  lib/api.ts                 จุดเรียก Tauri IPC จุดเดียว
  lib/workspace.ts           State ของแต่ละแท็บและการสะสม cell edits
  lib/types.ts               DTOs ฝั่ง UI
  lib/sample.ts              ข้อมูลตัวอย่างแยกจากฐานข้อมูลจริง
src-tauri/src/
  commands.rs                Thin Tauri command handlers
  service.rs                 Sessions, jobs, cancellation และ operation limits
  policy.rs                  SELECT allowlist / AST validation
  postgres.rs                Transport, preflight, catalog reads และ bounded queries
  mysql.rs                   MySQL SELECT-only adapter และ catalog queries
  redis_store.rs             Redis read-command allowlist และ typed responses
  remote_profile.rs          Session settings และ TLS/input validation
  bounded_stream.rs          Redis wire-byte limit ก่อน decode
  mutations.rs               Typed manual UPDATE, conflict checks และ atomic save
  models.rs                  DTOs และ safe errors
tests/                       Disposable database fixture และ browser UI tests
```

ใช้ Rust crate เดียว แยก modules ตามหน้าที่ ยังไม่สร้าง plugin framework หรือหลาย crates ที่ยังไม่มีเหตุผลในการแยก ใช้ `tokio-postgres` สำหรับ PostgreSQL เพราะมี native cancellation และ text-result streaming ที่รักษาความแม่นยำของ numeric/bigint; ไม่ดึง ORM เข้ามาเพื่อรัน SQL ของผู้ใช้

## ตรวจคุณภาพ

```sh
pnpm check
pnpm test
pnpm exec playwright install chromium webkit
pnpm test:ui
pnpm test:rust
pnpm lint:rust
pnpm format:check
```

Integration tests ต้องใช้ Compose fixture และรันแบบเรียงลำดับ เพราะมีการทดสอบเปลี่ยนสิทธิ์และ lock ตาราง:

```sh
docker compose up -d --wait
pnpm test:integration
```

สร้าง desktop build:

```sh
# macOS
pnpm tauri build --bundles app

# Windows, on a Windows machine with the required build tools
pnpm tauri build --bundles nsis
```

ผล build macOS อยู่ที่ `src-tauri/target/release/bundle/macos/buggy-table.app` เป็น local preview build ยังไม่มี Developer ID signing/notarization สำหรับเผยแพร่ทั่วไป Windows ต้อง build และตรวจ native behavior บน Windows จริงก่อนรับรอง

อ่านรายละเอียดการพัฒนาและขอบเขตความปลอดภัยใน [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md), ผลตรวจใน [docs/VALIDATION.md](docs/VALIDATION.md) และแผนเต็มใน [DATABASE_TOOL_PLAN.md](DATABASE_TOOL_PLAN.md)
