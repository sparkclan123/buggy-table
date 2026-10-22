# buggy-table

A personal, local-first database workspace built with **Rust, Tauri 2, Svelte and TypeScript**.

รุ่นนี้รองรับ **PostgreSQL 15–18, MySQL 8.x และ Redis standalone** โดย PostgreSQL อ่าน/แก้ไขเซลล์ได้ ส่วน MySQL และ Redis เปิดแบบอ่านอย่างเดียว มีหน้าตัวอย่างสำหรับลอง UI โดยไม่ต้องเชื่อมฐานข้อมูล ยังไม่ครบทุกข้อในแผนระยะยาว

## ใช้งานได้แล้ว

- เลือก PostgreSQL / MySQL / Redis ในหน้าต่อฐานข้อมูล พร้อม port เริ่มต้นตามชนิด
- OpenVPN ในตัวแอป แยกต่อ connection โดยไม่ต้องใช้ Docker: สวิตช์เปิด/ปิด, โปรไฟล์/default แยกแต่ละฐานข้อมูล, ดูสถานะ, Test route และ Reconnect โดยไม่เปลี่ยน routing/DNS ของเครื่อง ดูวิธีใช้และข้อจำกัดใน [Connection workspaces](docs/CONNECTIONS.md#openvpn-แยกต่อฐานข้อมูล)
- MySQL: ตารางหลายแท็บ, filter/sort, SQL SELECT, EXPLAIN, structure, indexes และ overview; ไม่บังคับชื่อ database ตอนเชื่อมต่อ กด + เพื่อเปิดตารางในแต่ละ database; บัญชี admin ใช้อ่านได้ด้วย
- Redis: SCAN คีย์ตาม pattern, ดูค่า/ชนิด/TTL, อ่าน string/hash/list/set/sorted set และรันคำสั่งอ่านแบบ JSON arguments
- เชื่อมทั้งสามชนิดด้วย verified TLS หรือ plaintext เฉพาะ literal loopback IP; PostgreSQL และ MySQL มีโหมด TLS แบบไม่ตรวจใบรับรองที่ต้องเลือกเอง และรองรับ CA PEM ส่วนตัว
- เปิดหลายตารางเป็นแท็บด้านบน เก็บ filter, page, SQL, scroll และ pending edits แยกกัน
- คลิกเซลล์ครั้งแรกเลือกและไฮไลต์ทั้งแถว คลิกเซลล์ในแถวที่เลือกอีกครั้งจึงแก้ไขได้: Enter จบการแก้ไข, Escape คืนค่าก่อนแก้, Shift+Enter ขึ้นบรรทัดใหม่; Save changes หรือ Cmd/Ctrl+S เพื่อบันทึก และ Discard เพื่อยกเลิก
- Cmd/Ctrl+Z ย้อนการแก้เซลล์ การเตรียมลบ หรือเพิ่มแถวที่ยังไม่ Save; Shift+Cmd/Ctrl+Z ทำซ้ำ ประวัติแยกแต่ละแท็บ จำกัด 100 ขั้น/8 MiB และล้างเมื่อ Save, Discard หรือโหลดข้อมูลใหม่
- เลือกแถวแล้ว Cmd/Ctrl+C และ Cmd/Ctrl+V ในตารางเดิม เพื่อเพิ่มสำเนาไว้ท้ายหน้าปัจจุบันเป็น draft สีเหลือง ต้อง Save จึงเขียนลง PostgreSQL; ไม่คัดลอก Primary key หรือ generated/identity columns ช่อง DEFAULT ให้ฐานข้อมูลสร้างค่า ส่วน Required ต้องกรอกเอง ต้องมีสิทธิ์ INSERT; copy/paste ภายในช่องพิมพ์ยังเป็นการคัดลอกข้อความตามปกติ
- ช่องแก้ไขข้อความยาวขยายเป็นกล่องหลายบรรทัดพร้อม scrollbar บาง โดยยังใช้ Enter/Escape และ Shift+Enter ได้
- ลากขอบหัวคอลัมน์เพื่อปรับความกว้าง ดับเบิลคลิกขอบเพื่อคืนขนาดเดิม หรือโฟกัสขอบแล้วกดลูกศรซ้าย/ขวา
- คลิกหัวคอลัมน์เพื่อ sort ASC/DESC จากฐานข้อมูลก่อนแบ่งหน้า; เก็บความกว้างและการเรียงแยกตามแท็บ
- เพิ่ม/ลบ filter ได้สูงสุด 20 เงื่อนไข เลือก AND/OR แล้ว Apply filters
- Cmd+R บน Mac, Ctrl+R บน Windows หรือ F5 รีเฟรชข้อมูลตาราง โดยไม่รีโหลดทั้งแอป; ถ้ามีค่ารอ Save จะเตือนก่อน
- Save เป็น transaction เดียว ตรวจ Primary key, INSERT/UPDATE/DELETE permission และค่าต้นฉบับก่อนบันทึก
- SQL editor ยังอ่านอย่างเดียว; Manual Save แยกจาก query/AI รองรับลบแถวที่เลือก ไม่มี Truncate endpoint
- UI กระชับ โทนเรียบ ข้อมูลไม่มี status สี, filter ใช้ bound parameters, pagination และ virtualized grid ตามขนาดหน้าจอ
- SQL editor พร้อม syntax highlighting, completion และ `Cmd/Ctrl + Enter`
- SELECT subset, estimated `EXPLAIN`, table structure, index definitions/scan counters
- Database overview จาก PostgreSQL จริง พร้อมเวลาที่เก็บและคำอธิบาย coverage
- Cancel, server/client deadlines, result row/byte caps และการปฏิเสธคำสั่งเขียนใน SQL editor
- SQL และ results อยู่ใน memory ของ session; รหัสผ่าน DB เก็บใน system password store เฉพาะเมื่อเลือก ส่วนโปรไฟล์/รหัสผ่าน VPN เก็บแบบเข้ารหัสในเครื่องด้วยกุญแจจาก system password store
- ไม่มี analytics, cloud AI, remote fonts หรือ download โมเดลอัตโนมัติ
- Light / Dark / System ที่ Appearance; จดจำเฉพาะค่า preference
- ปุ่ม Updates มุมล่างขวาเปิดหน้าดาวน์โหลด GitHub; รุ่น Preview ต้องติดตั้งทับเอง ส่วน signed release มี Update options สำหรับตรวจและติดตั้งในแอป
- แถวใน Data และ Row details สลับสีตามธีมมืด/สว่าง โดยสีแก้ไขและลบยังเด่นกว่า; เริ่มต้น 100 แถวต่อหน้า เลือก 50/100/250/500 ได้
- กดค้างหรือลากแท็บเพื่อจัดลำดับ มีภาพแท็บลอยและเลื่อนเมื่อชิดขอบ; Alt+←/→ ใช้แทนการลากได้ จำลำดับกับ connection ในเครื่องโดยไม่โหลดข้อมูลใหม่
- เมื่อแท็บล้นพื้นที่ ใช้ลูกกลิ้งเมาส์เลื่อนซ้าย–ขวาได้ และยังใช้แทร็กแพดแนวนอนได้
- วิธีสร้างตัวติดตั้งและออกรุ่นใหม่: [Distribution guide](docs/DISTRIBUTION.md)

Primary key ของแถวเดิม, identity/generated columns และตารางไม่มี Primary key ยังแก้ไม่ได้ ส่วนแถวใหม่กรอก Primary key ที่ไม่ได้สร้างอัตโนมัติได้ Save จำกัด 100 แถว/500 เซลล์/4 MiB ต่อครั้ง หากเกิด conflict จะไม่บันทึกทั้งชุด หากผล COMMIT ไม่แน่นอน จะบล็อกการบันทึกซ้ำจน discard draft และ reload ทั้งนี้ manual INSERT/UPDATE/DELETE ทำงานตาม constraints/triggers ของฐานข้อมูลตามปกติ

- จำ connection ในเครื่อง พร้อมกลุ่ม/ปักหมุด/ค้นหา (`⌘/Ctrl K`) และ Reconnect หลังเปิดแอปใหม่ เก็บรหัสผ่านใน OS password store ได้เมื่อเลือกเอง
- สลับ PostgreSQL schema และหลาย connection โดยรักษาแท็บ/เซลล์ที่แก้ค้าง แสดงรายการล่าสุด 5 รายการ และค้นหาทั้งหมดได้
- PostgreSQL แสดงรายชื่อ tables/views ใน schema พร้อมเหตุผลสำหรับรายการที่อ่านไม่ได้; รองรับ JSON/JSONB, enum และ array ของชนิดที่รองรับ รวมถึงอ่าน materialized views และมีปุ่ม Refresh schema
- อ่าน catalog เป็นชุดแทนการส่งคำสั่งแยกทุกตาราง เพื่อลดเวลารอเซิร์ฟเวอร์ภายนอก; เวลา PostgreSQL แสดงผลรวมพร้อมรายละเอียด connection/checks/query เมื่อชี้ที่ตัวเลข

- ช่องรอ Save มีพื้นหลังไฮไลต์; คลิกเลขแถว # แล้ว Delete/Backspace เพื่อเตรียมลบเป็นสีแดง และ Restore/Discard เพื่อคืนแถว; ต้องมี Primary key และสิทธิ์ DELETE
- ลากหัวคอลัมน์สลับตำแหน่ง หรือ Alt+ลูกศรซ้าย/ขวา; บันทึกตำแหน่งเฉพาะเครื่องนี้ ไม่เปลี่ยน schema
- Timestamp พิมพ์เองหรือใช้ปฏิทิน/เวลาได้ทั้งการแก้ไขและ filter โดยรักษา offset และเศษวินาที; filter ใช้สัญลักษณ์พร้อมคำอธิบายเมื่อชี้เมาส์
- แบ่งหน้า 100 รายการเริ่มต้น เลือก 50/100/250/500 ได้; แสดงจำนวนประมาณก่อน และ Count exact สำหรับนับตาม filter โดยมี query timeout 5 วินาที
- Full values เปิดการตัดบรรทัดเพื่อดูค่าเต็มในหน้าปัจจุบัน (ปิดเป็นค่าเริ่มต้น); คอลัมน์ # ตรึงเมื่อเลื่อนแนวนอน
- Row details ข้าง Full values เปิด/ปิดแถบดูข้อมูลด้านขวา: เริ่มกว้าง 10% ของพื้นที่ตาราง ลากขยาย/ย่อและปิดด้วย × ได้ จดจำค่าเปิด–ปิดและความกว้างเฉพาะเครื่อง แสดงค่าเต็มเรียงแนวตั้ง เลื่อนดูได้ และจัด JSON/JSONB ให้อ่านง่ายโดยคงตัวเลข/ทศนิยมเดิม; ใช้ข้อมูลที่โหลดแล้ว ไม่ query เพิ่ม
- สีแก้ไขและสีแดงเตรียมลบเด่นกว่าสีเลือกแถว; แถบ Row details อ่านอย่างเดียวและแสดง draft ล่าสุด โดยไม่บันทึกข้อมูลแถวลง storage
- ปุ่มข้าง Refresh หุบ/กางแถบเครื่องมือ; Cmd/Ctrl+W ปิดแท็บปัจจุบันพร้อมตรวจงานค้าง; scrollbar แท็บเล็กลง
- PostgreSQL ใช้ connection เดิมต่อสำหรับการอ่าน และส่ง metadata checks แบบ pipelining; SQL ตรวจเฉพาะตารางที่อ้างถึง พร้อมแสดงเวลาแต่ละช่วงและสถานะ connection reuse
- Overview PostgreSQL/MySQL และ Redis INFO แสดงกราฟจาก snapshot จริง เก็บประวัติ 30 ครั้งในหน่วยความจำจากการ refresh ด้วยตนเอง
- Redis SCAN แสดงแต่ละคีย์เป็นแถว รวมผล Next scan โดยตัดคีย์ซ้ำ; ค้นหาด้วย Enter และคำสั่ง SCAN ให้ผลรูปแบบเดียวกัน
- MySQL รองรับ CA PEM ส่วนตัว และแยกข้อความผิดพลาดการเชื่อมต่อเป็น network/TLS/auth/permission/server compatibility

**ยังไม่เปิดใช้:** MySQL/Redis manual writes, Redis Cluster/Sentinel/streams, MariaDB, local AI, SSH, query history, joins/CTEs/functions/casts และการรับรองใช้งาน production; PostgreSQL views/RLS/foreign tables/partitioned parents/virtual generated columns และชนิด domain/composite/extension แสดงโครงสร้างพร้อมเหตุผล แต่ยังอ่านข้อมูลไม่ได้ ส่วน materialized views อ่านได้

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

เลือก Local only สำหรับ fixture เท่านั้น สำหรับเครื่องอื่นใช้ verified TLS และ certificate ที่ระบบเชื่อถือ; PostgreSQL รองรับ custom CA และโหมด TLS แบบเข้ารหัสโดยไม่ตรวจใบรับรอง ดู [การแก้ปัญหาเชื่อมต่อ](docs/CONNECTIONS.md) MySQL รองรับบัญชี reader หรือ admin โดยแอปยังอ่านอย่างเดียว เลือกใช้ reader เพื่อจำกัดสิทธิ์ที่เซิร์ฟเวอร์เพิ่มเติมได้ ส่วน Redis ควรใช้ ACL user จำกัดคำสั่ง ไม่ต้องให้สิทธิ์ admin; แอปบล็อกคำสั่งเขียน/ลบเองด้วย

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
  mutations.rs               Typed manual INSERT/UPDATE/DELETE, conflict checks และ atomic save
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
