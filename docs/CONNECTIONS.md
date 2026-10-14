# Connection workspaces

Database คือชื่อฐานข้อมูลจริง ส่วน Schema มักเป็น `public` แต่ใช้ค่าอื่นได้ตามฐานข้อมูล ไม่จำเป็นต้องเป็นชื่อเดียวกัน

## เวอร์ชัน PostgreSQL

ตั้งแต่ buggy-table 0.1.2 รองรับและทดสอบ PostgreSQL 15, 16, 17 และ 18 รุ่นก่อน 15 ไม่อยู่ในขอบเขต ส่วนรุ่นใหม่กว่า 18 จะลองเชื่อมต่อได้โดยไม่บล็อกเลขเวอร์ชัน แต่แสดงข้อความว่ายังไม่ผ่านการทดสอบ ข้อจำกัด SQL และสิทธิ์ยังคงทำงานตามเดิม

ถ้าเห็น “This preview enables PostgreSQL 17” แสดงว่ากำลังใช้แอปรุ่นเก่า ให้ปิดแอปแล้วติดตั้ง 0.1.2 หรือใหม่กว่า ไม่จำเป็นต้องเปลี่ยนเวอร์ชันฐานข้อมูลในช่วงที่รองรับ

## เลือก TLS ให้ตรงเซิร์ฟเวอร์

- **TLS · verify certificate and hostname** เป็นค่าเริ่มต้น ตรวจ CA และชื่อ Host สำหรับ private CA เปิด Custom CA certificate แล้วเลือกไฟล์ `.pem` / `.crt` จากผู้ให้บริการฐานข้อมูล ใบรับรองจะจำไว้พร้อม connection ถ้าเปิด Remember connectionและยังต้องใช้ Host ที่ตรง certificate รองรับ PEM bundle ไม่เกิน 64 KiB / 16 certificates ไม่รับ private key
- **TLS · encrypted, certificate not verified** สำหรับ PostgreSQL ที่เข้ารหัสได้ แต่ใช้ใบรับรองที่เครื่องไม่เชื่อถือหรือชื่อไม่ตรง Host เทียบกับ `sslmode=require` โดยไม่ตรวจ certificate/hostname ต้องเลือกเอง การเชื่อมต่อยังเข้ารหัส แต่ไม่ป้องกันการปลอมเป็นเซิร์ฟเวอร์ ไม่มี fallback ไป plaintext
- **Local only** ใช้ได้เฉพาะ IP `127.0.0.1` / `::1` สำหรับฐานข้อมูลในเครื่องหรือ trusted tunnel ไม่ใช้กับ public IP

`PREFERRED` ใน client อื่นไม่ได้บังคับตรวจใบรับรองแบบ verify-full และอาจยอมเชื่อมแบบไม่เข้ารหัสเมื่อเซิร์ฟเวอร์ไม่รองรับ TLS ดังนั้น client อื่นเชื่อมต่อได้ ไม่ได้แปลว่า verify-full จะผ่านด้วย

สำหรับ Google Cloud SQL ที่ใช้ per-instance CA สามารถใช้ Cloud SQL Auth Proxy เพื่อให้ proxy จัดการ TLS/identity แล้วเชื่อมผ่าน loopback ได้ ส่วนการเชื่อมโดยตรงขึ้นกับ CA, hostname และการบังคับ client certificates ของ instance รุ่นนี้ยังไม่รองรับ client-certificate authentication

## เมื่อเกิด error

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
