# PostgreSQL connection settings

Database คือชื่อฐานข้อมูลจริง ส่วน Schema มักเป็น `public` แต่ใช้ค่าอื่นได้ตามฐานข้อมูล ไม่จำเป็นต้องเป็นชื่อเดียวกัน

## เวอร์ชัน PostgreSQL

ตั้งแต่ buggy-table 0.1.2 รองรับและทดสอบ PostgreSQL 15, 16, 17 และ 18 รุ่นก่อน 15 ไม่อยู่ในขอบเขต ส่วนรุ่นใหม่กว่า 18 จะลองเชื่อมต่อได้โดยไม่บล็อกเลขเวอร์ชัน แต่แสดงข้อความว่ายังไม่ผ่านการทดสอบ ข้อจำกัด SQL และสิทธิ์ยังคงทำงานตามเดิม

ถ้าเห็น “This preview enables PostgreSQL 17” แสดงว่ากำลังใช้แอปรุ่นเก่า ให้ปิดแอปแล้วติดตั้ง 0.1.2 หรือใหม่กว่า ไม่จำเป็นต้องเปลี่ยนเวอร์ชันฐานข้อมูลในช่วงที่รองรับ

## เลือก TLS ให้ตรงเซิร์ฟเวอร์

- **TLS · verify certificate and hostname** เป็นค่าเริ่มต้น ตรวจ CA และชื่อ Host สำหรับ private CA เปิด Custom CA certificate แล้วเลือกไฟล์ `.pem` / `.crt` จากผู้ให้บริการฐานข้อมูล ใบรับรองอยู่ใน session เท่านั้นและยังต้องใช้ Host ที่ตรง certificate รองรับ PEM bundle ไม่เกิน 64 KiB / 16 certificates ไม่รับ private key
- **TLS · encrypted, certificate not verified** สำหรับ PostgreSQL ที่เข้ารหัสได้ แต่ใช้ใบรับรองที่เครื่องไม่เชื่อถือหรือชื่อไม่ตรง Host เทียบกับ `sslmode=require` โดยไม่ตรวจ certificate/hostname ต้องเลือกเอง การเชื่อมต่อยังเข้ารหัส แต่ไม่ป้องกันการปลอมเป็นเซิร์ฟเวอร์ ไม่มี fallback ไป plaintext
- **Local only** ใช้ได้เฉพาะ IP `127.0.0.1` / `::1` สำหรับฐานข้อมูลในเครื่องหรือ trusted tunnel ไม่ใช้กับ public IP

`PREFERRED` ใน client อื่นไม่ได้บังคับตรวจใบรับรองแบบ verify-full และอาจยอมเชื่อมแบบไม่เข้ารหัสเมื่อเซิร์ฟเวอร์ไม่รองรับ TLS ดังนั้น client อื่นเชื่อมต่อได้ ไม่ได้แปลว่า verify-full จะผ่านด้วย

สำหรับ Google Cloud SQL ที่ใช้ per-instance CA สามารถใช้ Cloud SQL Auth Proxy เพื่อให้ proxy จัดการ TLS/identity แล้วเชื่อมผ่าน loopback ได้ ส่วนการเชื่อมโดยตรงขึ้นกับ CA, hostname และการบังคับ client certificates ของ instance รุ่นนี้ยังไม่รองรับ client-certificate authentication

## เมื่อเกิด error

ข้อความใหม่แยก TLS failure, network refusal/timeout, authentication และ database not found หากแก้ TLS แล้วขึ้น authentication failure ให้กรอกรหัสผ่านอีกครั้ง แอปล้างช่อง Password หลังทุกครั้งที่พยายามเชื่อมต่อ และไม่บันทึกรหัสผ่านลงดิสก์

อ้างอิง: [PostgreSQL SSL modes](https://www.postgresql.org/docs/current/libpq-ssl.html), [Cloud SQL certificates](https://docs.cloud.google.com/sql/docs/postgres/configure-ssl-instance)
