# Connection workspaces

สำหรับ PostgreSQL: Database คือชื่อฐานข้อมูลจริง ส่วน Schema มักเป็น `public` แต่ใช้ค่าอื่นได้ตามฐานข้อมูล ไม่จำเป็นต้องเป็นชื่อเดียวกัน

## MySQL: เลือกฐานข้อมูลหลังเชื่อมต่อ — 0.1.14

ช่อง **Default database (optional)** เว้นว่างได้ กรอก Host, Port, Username และ Password แล้วเชื่อมต่อ แอปจะแสดงฐานข้อมูลที่บัญชีมองเห็นในแถบซ้าย กด **+** หน้าชื่อเพื่อโหลดและขยายตาราง กด **−** เพื่อหุบ รายการที่โหลดแล้วใช้ซ้ำภายใน session; ปุ่ม refresh ข้างฐานข้อมูลโหลดตารางใหม่ ส่วนปุ่ม refresh ข้าง Databases โหลดรายชื่อฐานข้อมูลใหม่ Search ค้นชื่อฐานข้อมูลและตารางที่โหลดแล้ว

ตารางชื่อเดียวกันในคนละฐานข้อมูลเปิดแยกแท็บได้ Browse, filters, counts และ indexes ยึดฐานข้อมูลของแท็บนั้น SQL ต้องใช้ชื่อแบบ `database.table` ถ้า Remember connection เปิดอยู่ สามารถบันทึก connection ที่ไม่มี default database และคืนแท็บเดิมหลัง reconnect ได้

บัญชี admin และบัญชีที่มีสิทธิ์เขียนเชื่อมต่อได้ แต่ MySQL ในแอปยังอ่านอย่างเดียว ทุก session ตั้ง transaction_read_only และ autocommit พร้อมตรวจคำสั่ง SELECT ก่อนส่งไปเซิร์ฟเวอร์ อ่านสำเร็จแล้วใช้ connection เดิมซ้ำโดยไม่ค้าง transaction Views, generated columns, storage engines และชนิดข้อมูลที่ยังอ่านไม่ได้จะแสดงโครงสร้างพร้อมเหตุผล ไม่ซ่อนตารางทั้งรายการ

อ้างอิง: [MySQL default database is optional](https://dev.mysql.com/doc/refman/8.4/en/connecting.html), [read-only transactions](https://dev.mysql.com/doc/refman/8.4/en/commit.html)

## เวอร์ชัน PostgreSQL

ตั้งแต่ buggy-table 0.1.2 รองรับและทดสอบ PostgreSQL 15, 16, 17 และ 18 รุ่นก่อน 15 ไม่อยู่ในขอบเขต ส่วนรุ่นใหม่กว่า 18 จะลองเชื่อมต่อได้โดยไม่บล็อกเลขเวอร์ชัน แต่แสดงข้อความว่ายังไม่ผ่านการทดสอบ ข้อจำกัด SQL และสิทธิ์ยังคงทำงานตามเดิม

ถ้าเห็น “This preview enables PostgreSQL 17” แสดงว่ากำลังใช้แอปรุ่นเก่า ให้ปิดแอปแล้วติดตั้ง 0.1.2 หรือใหม่กว่า ไม่จำเป็นต้องเปลี่ยนเวอร์ชันฐานข้อมูลในช่วงที่รองรับ

## เลือก TLS ให้ตรงเซิร์ฟเวอร์

- **TLS · verify certificate and hostname** เป็นค่าเริ่มต้น ตรวจ CA และชื่อ Host สำหรับ private CA เปิด Custom CA certificate แล้วเลือกไฟล์ `.pem` / `.crt` จากผู้ให้บริการฐานข้อมูล ใบรับรองจะจำไว้พร้อม connection ถ้าเปิด Remember connectionและยังต้องใช้ Host ที่ตรง certificate รองรับ PEM bundle ไม่เกิน 64 KiB / 16 certificates ไม่รับ private key
- **TLS · encrypted, certificate not verified** สำหรับ PostgreSQL หรือ MySQL ที่เข้ารหัสได้ แต่ใช้ใบรับรองที่เครื่องไม่เชื่อถือหรือชื่อไม่ตรง Host เทียบกับ PostgreSQL `sslmode=require` / MySQL `ssl-mode=REQUIRED` โดยไม่ตรวจ certificate/hostname ต้องเลือกเอง การเชื่อมต่อยังเข้ารหัส แต่ไม่ป้องกันการปลอมเป็นเซิร์ฟเวอร์ ไม่มี fallback ไป plaintext
- **Local only** ใช้ได้เฉพาะ IP `127.0.0.1` / `::1` สำหรับฐานข้อมูลในเครื่องหรือ trusted tunnel ไม่ใช้กับ public IP

`PREFERRED` ใน client อื่นไม่ได้บังคับตรวจใบรับรองแบบ verify-full และอาจยอมเชื่อมแบบไม่เข้ารหัสเมื่อเซิร์ฟเวอร์ไม่รองรับ TLS ดังนั้น client อื่นเชื่อมต่อได้ ไม่ได้แปลว่า verify-full จะผ่านด้วย

สำหรับ Google Cloud SQL ที่ใช้ per-instance CA สามารถใช้ Cloud SQL Auth Proxy เพื่อให้ proxy จัดการ TLS/identity แล้วเชื่อมผ่าน loopback ได้ ส่วนการเชื่อมโดยตรงขึ้นกับ CA, hostname และการบังคับ client certificates ของ instance รุ่นนี้ยังไม่รองรับ client-certificate authentication

## เมื่อเกิด error

MySQL ที่ขึ้น “TLS verification failed” ให้ตรวจ Host ว่าตรงชื่อในใบรับรอง, วันหมดอายุ และ CA ก่อน ถ้าใช้ private CA ให้เลือก PEM และคงโหมด verify certificate and hostname ไว้ หากตั้งใจใช้เซิร์ฟเวอร์ self-signed ที่ยังไม่มี CA ที่เชื่อถือ เลือก encrypted, certificate not verified ได้โดยยอมรับความเสี่ยงด้านการยืนยันตัวตนเซิร์ฟเวอร์ แอปไม่ลดระดับ TLS ให้อัตโนมัติ โหมดนี้ไม่ช่วยกรณีเซิร์ฟเวอร์บังคับ client certificate ซึ่งยังไม่รองรับ

ข้อความใหม่แยก TLS failure, network refusal/timeout, authentication และ database not found หากแก้ TLS แล้วขึ้น authentication failure ให้กรอกรหัสผ่านอีกครั้ง แอปล้างช่อง Password หลังทุกครั้งที่พยายามเชื่อมต่อ และไม่ใส่รหัสผ่านในไฟล์ connection; หากเลือก Save password จะเก็บผ่าน OS password store

อ้างอิง: [PostgreSQL SSL modes](https://www.postgresql.org/docs/current/libpq-ssl.html), [Cloud SQL certificates](https://docs.cloud.google.com/sql/docs/postgres/configure-ssl-instance)


## หลาย connection / schema — 0.1.3

- กด **+** ที่ Connections เพื่อเพิ่ม PostgreSQL, MySQL หรือ Redis โดยไม่ตัดการเชื่อมต่อเดิม
- แถบซ้ายแสดงปักหมุด/ล่าสุด 5 รายการ กด **All connections** หรือ **⌘K / Ctrl+K** เพื่อค้นหาชื่อ host database หรือ group กรอง engine/group ได้ และแบ่งรายการหน้าละ 12
- เลือก connection ที่พร้อมใช้งานเพื่อสลับทันที แต่ละ connection แยกแท็บ ตัวกรอง SQL ที่พิมพ์ และเซลล์ที่แก้ค้างไว้ สัญลักษณ์จุดแสดงงานที่ยังไม่ Save
- PostgreSQL: เลือก dropdown Schema ที่ด้านล่าง sidebar เลือกได้เฉพาะ schema ที่บัญชีมี USAGE ตารางชื่อซ้ำต่าง schema เปิดคนละแท็บ Save/SQL/Index ยึด schema ของแท็บ ไม่ย้ายตามรายการด้านซ้าย
- **Remember connection on this device** จำชื่อ endpoint engine username TLS/CA กลุ่ม ปักหมุด และชื่อแท็บใน local app data (`connections.json`) ไม่บันทึกรหัสผ่าน SQL ตัวกรอง ผลลัพธ์ หรือเซลล์ที่แก้ค้าง
- **Save password in system password store** เป็นตัวเลือกแยก (ค่าเริ่มต้นปิด) ใช้ login Keychain บน Mac และ Credential Manager แบบ Local บน Windows แอปไม่เปิดการ sync ของรหัสผ่าน
- เปิดแอปใหม่จะแสดงรายการเดิมแบบ Disconnected ไม่เชื่อมเอง กด **Reconnect**; ถ้าไม่ได้เก็บรหัสผ่านหรือ password store ถูกล็อก จะเปิดฟอร์มให้กรอกรหัสผ่าน
- Disconnect คืนทรัพยากรและล้างผลลัพธ์ในหน่วยความจำ แต่ยังเก็บ bookmark/password ที่เลือกไว้ การลบ bookmark ทำได้หลัง Disconnect และจะลบรหัสผ่านของ bookmark นั้นด้วย ไม่แตะฐานข้อมูล
- ต้อง Save/Discard ก่อนปิดแอป อัปเดต หรือตัด connection ที่มีงานค้าง การสลับไป connection อื่นทำได้โดยงานเดิมไม่หาย

ขีดจำกัดรุ่นนี้: 500 bookmarks, 8 sessions พร้อมใช้งาน, 40 แท็บต่อ connection และเก็บผลลัพธ์ของแท็บสะอาดล่าสุดประมาณ 6 แท็บ (แท็บที่มีงานแก้ค้างไม่ถูกทิ้ง) กลับไปแท็บที่ถูกคืนหน่วยความจำจะโหลดข้อมูลใหม่ Session หมายถึงโปรไฟล์ที่พร้อมส่งคำสั่ง; adapter เปิด socket ตาม operation ไม่ได้ค้าง socket ทั้ง 8 ตลอดเวลา

ไฟล์ local ไม่ถูกส่งโดยแอปไป cloud แต่การสำรองข้อมูลของระบบปฏิบัติการหรือซอฟต์แวร์อื่นอยู่นอกการควบคุมของแอป รหัสผ่านที่ไม่ได้เลือกเก็บใช้เฉพาะ session และไม่ถูกส่งกลับมายัง frontend เมื่อ Reconnect

ตั้งแต่ 0.1.4 ตัวปรับธีมอยู่ที่ไอคอนข้างชื่อแอป/เลขเวอร์ชันด้านล่าง เลือก System, Light หรือ Dark ได้เหมือนเดิม

## OpenVPN แยกต่อฐานข้อมูล

**ตั้งแต่ 0.1.16 ไม่ต้องติดตั้ง Docker, OpenVPN client หรือ driver เพิ่ม** ตัวติดตั้งรวม OpenVPN engine และระบบเครือข่ายเฉพาะแอปไว้แล้ว รองรับ Mac/Windows โปรไฟล์ที่บันทึกไว้ใช้ต่อได้โดยไม่ต้องนำเข้าใหม่

1. กดรูปตั้งค่าข้างสวิตช์ **VPN** ใน connection หรือในฟอร์มต่อฐานข้อมูล
2. กด **Add VPN** เลือกไฟล์ `.ovpn` แบบ inline (CA/certificate/key อยู่ในไฟล์เดียว) ตั้งชื่อ และกรอก VPN username/password หากเซิร์ฟเวอร์ต้องใช้ เพิ่มได้หลายโปรไฟล์ต่อ connection
3. เลือก radio ของโปรไฟล์เพื่อกำหนด default เปิด **Use VPN for this database** แล้ว Apply การตั้งค่าจะจำคู่กับ bookmark บนเครื่องนี้ เปิดแอปใหม่ยังไม่เชื่อมอัตโนมัติ; เมื่อกด Reconnect จะต่อ VPN ก่อนตรวจสิทธิ์ฐานข้อมูล
4. ใส่ Host ของฐานข้อมูลเป็น **ชื่อหรือ IP ภายใน VPN** ไม่ใช่ localhost หาก VPN ไม่ส่ง DNS มา ให้ใส่ private DNS IP ใน Advanced หรือใช้ IP ของฐานข้อมูล

หน้า settings แสดง **OpenVPN ready** โดยไม่ต้อง Prepare หรือดาวน์โหลดเพิ่ม แต่ละ VPN มี process และ TCP/IP stack ของตนเอง คง tunnel ไว้ตลอด session ไม่ handshake ใหม่ทุก query เมื่อ Disconnect จะหยุด process ของ connection นั้น

สวิตช์ปิดใช้เครือข่ายปกติของเครื่อง เปิดใช้ VPN ของ connection นั้น แต่ละ tunnel แยก routing/DNS จึงใช้ฐานข้อมูลคนละ VPN ที่มี private IP หรือชื่อซ้ำกันได้ การต่อ VPN ไม่เปลี่ยนโหมด TLS ของฐานข้อมูล: verified TLS ยังคงตรวจชื่อ Host เดิม ส่วน private CA ของ DB ต้องตั้งแยกจาก CA ใน `.ovpn`

**สถานะและ refresh:** จุดสถานะแสดง tunnel ready / disconnected / destination unreachable ใน VPN settings มีผลตรวจและเวลาล่าสุด **Test route** ตรวจ DNS/TCP ไปยังพอร์ตฐานข้อมูลผ่าน VPN และแสดง latency ไม่รัน SQL และไม่ได้ยืนยันว่า login/query จะผ่าน การอ่านสถานะอัตโนมัติทุก 15 วินาทีไม่ส่ง query หรือ health probe เพิ่ม การตรวจสายขาดของ OpenVPN ขึ้นกับค่า keepalive ของ VPN server และรอบตรวจสถานะ กด **Reconnect** หรือไอคอน refresh ข้าง VPN เพื่อต่อทั้ง VPN และฐานข้อมูลของ connection นั้นใหม่

ต้อง Save/Discard งานแก้ค้างก่อนเปลี่ยน network หรือ Reconnect แอปตรวจ connection ใหม่ก่อนแทน session เดิม หากล้มเหลวจะไม่ล้าง workspace เดิม VPN ที่หลุด **ไม่ fallback** ไปเครือข่ายหลัก การใช้งานใบรับรอง client เดียวพร้อมกันหลาย connection รวมถึงช่วง refresh ขึ้นกับข้อจำกัดของ VPN server; บาง server ตัด session เก่าเมื่อ client เดียวกันต่อใหม่ ให้ใช้ credential ที่อนุญาต concurrent sessions หรือแยก client certificates

เพิ่ม/แก้ชื่อ/ลบโปรไฟล์ได้ใน settings ต้อง Disconnect ก่อนแก้หรือลบโปรไฟล์ที่กำลังใช้งาน การลบ default จะปิด VPN preference ของ connection นั้นด้วย การลบ bookmark ลบโปรไฟล์ VPN ที่เป็นเจ้าของด้วย; การปิดฟอร์ม connection ใหม่ที่ยังไม่ได้เชื่อมจะล้าง draft profiles ที่สร้างในฟอร์มนั้น

ไฟล์ `.ovpn`, private keys และ VPN passwords เก็บแบบ AES-256-GCM ใน `vpn-profiles.json` โดยกุญแจอยู่ใน macOS login Keychain / Windows Credential Manager (Local) ชื่อโปรไฟล์/username/DNS/ความสัมพันธ์กับ connection เป็น metadata ที่ไม่ได้เข้ารหัส ไม่เก็บ secrets ใน browser storage, process arguments/environment หรือ logs และไม่ส่ง secret เดิมกลับมาให้ UI ช่อง password ว่างตอนแก้หมายถึงเก็บค่าเดิม; เลือก Clear เพื่อล้างรหัสผ่านที่บันทึกไว้

ขอบเขตรุ่นนี้: OpenVPN TUN, inline client profile ไม่เกิน 512 KiB และ 50 โปรไฟล์รวม รองรับ username/password และ encrypted private key password ไม่รองรับ TAP, `<connection>` blocks, external certificate/key files, user scripts/plugins, browser SSO หรือ interactive MFA/challenge หากไฟล์มี option ที่ยังไม่รองรับ แอปแจ้งชื่อ option แทนการละเลยเงียบ ๆ การยืนยันกับ VPN ภายนอกแต่ละผู้ให้บริการยังต้องทดสอบกับโปรไฟล์จริงของผู้ใช้

ตัวเลือก `push-peer-info` ใช้ส่งข้อมูล client เพิ่มเติมให้ VPN server ตาม [คู่มือ OpenVPN](https://openvpn.net/community-docs/community-articles/openvpn-2-6-manual.html) และไม่ใช่ script/plugin ตัวนำเข้าใน Preview 0.1.11 ปฏิเสธตัวเลือกนี้โดยผิดพลาด ตั้งแต่ 0.1.12 แก้ให้รับและคงตัวเลือกไว้แล้ว โดยไม่ต้องลบออกจากไฟล์ `.ovpn` หากพบ `Unsupported OpenVPN option` สำหรับตัวเลือกอื่น หมายถึงแอปยังไม่รองรับตัวเลือกนั้น ไม่ได้ยืนยันว่าไฟล์มีอันตราย

### เมื่อ VPN engine ไม่พร้อม

กด **Check VPN engine again** หากแอปแจ้งว่า bundled engine หายหรือเปิดไม่ได้ ให้ติดตั้งแอปรุ่นล่าสุดให้ตรงกับเครื่อง ไฟล์ engine ต้องอยู่ใน app bundle/โฟลเดอร์ติดตั้งเดียวกับแอป ไม่ต้องติดตั้ง Docker เพื่อแก้ปัญหานี้

ถ้า engine พร้อมแต่เชื่อมต่อไม่ได้ ให้ตรวจโปรไฟล์ CA/วันหมดอายุ VPN username/password และการเข้าถึง VPN server หาก tunnel พร้อมแต่ Test route ไม่ผ่าน ให้ตรวจ DNS ภายในและพอร์ตของฐานข้อมูล แอปไม่เปลี่ยนไปใช้เครือข่ายปกติโดยอัตโนมัติ
