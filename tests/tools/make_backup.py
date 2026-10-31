"""Build a small fake iPhone/iPad backup to test RecovMe's backup reader.

Usage:  python3 -I make_backup.py <out-folder> plain|enc
Encrypted backups use the password "secret" (with cheap key-derivation rounds,
so tests run fast). Needs: pip install cryptography
"""
import datetime, hashlib, os, plistlib, sqlite3, struct, sys
from cryptography.hazmat.primitives import padding
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives.keywrap import aes_key_wrap

out, encrypted = sys.argv[1], sys.argv[2] == "enc"
PW = b"secret"
os.makedirs(out, exist_ok=True)


def jpeg(n):  # a fake JPEG, big enough to be shown as a preview
    return b"\xff\xd8\xff\xe0" + bytes([n % 251]) * 30000 + b"\xff\xd9"


def enc(key, data):
    p = padding.PKCS7(128).padder()
    data = p.update(data) + p.finalize()
    e = Cipher(algorithms.AES(key), modes.CBC(b"\0" * 16)).encryptor()
    return e.update(data) + e.finalize()


def pbk(h, pw, salt, it):
    return hashlib.pbkdf2_hmac(h, pw, salt, it, 32)


def sqlite_bytes(name, script, rows=()):
    path = os.path.join(out, name)
    if os.path.exists(path):
        os.remove(path)
    c = sqlite3.connect(path)
    c.executescript(script)
    for sql, args in rows:
        c.execute(sql, args)
    c.commit()
    c.close()
    data = open(path, "rb").read()
    os.remove(path)
    return data


# Photos library: IMG_0002 was in Recently Deleted (original name beach.jpg),
# IMG_0004 is hidden (secret.jpg), IMG_0005 is only in iCloud (preview only).
photos = sqlite_bytes("_photos.sqlite", """
CREATE TABLE ZASSET (Z_PK INTEGER PRIMARY KEY, ZDIRECTORY TEXT, ZFILENAME TEXT,
  ZTRASHEDSTATE INTEGER, ZHIDDEN INTEGER, ZADDITIONALATTRIBUTES INTEGER);
CREATE TABLE ZADDITIONALASSETATTRIBUTES (Z_PK INTEGER PRIMARY KEY, ZORIGINALFILENAME TEXT);
INSERT INTO ZASSET VALUES (1,'DCIM/100APPLE','IMG_0001.JPG',0,0,1);
INSERT INTO ZASSET VALUES (2,'DCIM/100APPLE','IMG_0002.JPG',1,0,2);
INSERT INTO ZASSET VALUES (3,'DCIM/100APPLE','IMG_0004.JPG',0,1,3);
INSERT INTO ZASSET VALUES (4,'DCIM/100APPLE','IMG_0005.HEIC',0,0,4);
INSERT INTO ZADDITIONALASSETATTRIBUTES VALUES (1,'IMG_0001.JPG');
INSERT INTO ZADDITIONALASSETATTRIBUTES VALUES (2,'beach.jpg');
INSERT INTO ZADDITIONALASSETATTRIBUTES VALUES (3,'secret.jpg');
""")

# Messages: two messages in Recently Deleted (iOS 16+); the first one's text is only
# in attributedBody (a typedstream), the second has a photo attached.
body = (b"\x04\x0bstreamtyped\x81\xe8\x03\x84\x01@\x84\x84\x84\x12NSAttributedString\x00"
        b"\x84\x84\x08NSObject\x00\x85\x92\x84\x84\x84\x08NSString\x01\x94\x84\x01+\x0cSee you soon\x86")
sms = sqlite_bytes("_sms.db", """
CREATE TABLE message (ROWID INTEGER PRIMARY KEY, text TEXT, attributedBody BLOB, date INTEGER,
  is_from_me INTEGER, handle_id INTEGER);
CREATE TABLE handle (ROWID INTEGER PRIMARY KEY, id TEXT);
CREATE TABLE chat_recoverable_message_join (chat_id INTEGER, message_id INTEGER, delete_date INTEGER);
CREATE TABLE attachment (ROWID INTEGER PRIMARY KEY, filename TEXT);
CREATE TABLE message_attachment_join (message_id INTEGER, attachment_id INTEGER);
INSERT INTO handle VALUES (1, '+15551234567');
INSERT INTO attachment VALUES (1, '~/Library/SMS/Attachments/cd/02/XYZ/IMG_9.jpeg');
INSERT INTO message_attachment_join VALUES (2, 1);
INSERT INTO chat_recoverable_message_join VALUES (1,1,0),(1,2,0);
""", [("INSERT INTO message VALUES (1, NULL, ?, 700000000000000000, 0, 1)", (body,)),
      ("INSERT INTO message VALUES (2, 'Look at this', NULL, 700000100000000000, 1, 1)", ())])

# Voice Memos titles.
voice = sqlite_bytes("_vm.db", """
CREATE TABLE ZCLOUDRECORDING (ZPATH TEXT, ZCUSTOMLABELFORSORTING TEXT);
INSERT INTO ZCLOUDRECORDING VALUES ('/var/mobile/Recordings/20240101 120000.m4a', 'Grandma birthday song');
""")

# (domain, relativePath, content, protection class)
files = [
    ("CameraRollDomain", "Media/DCIM/100APPLE/IMG_0001.JPG", jpeg(1), 3),
    ("CameraRollDomain", "Media/DCIM/100APPLE/IMG_0002.JPG", jpeg(2), 3),
    ("CameraRollDomain", "Media/DCIM/100APPLE/IMG_0003.MOV", b"\0\0\0\x14ftypqt  " + b"m" * 5000, 3),
    ("CameraRollDomain", "Media/DCIM/100APPLE/IMG_0004.JPG", jpeg(4), 3),
    ("CameraRollDomain", "Media/PhotoData/Thumbnails/V2/DCIM/100APPLE/IMG_0005.HEIC/5005.JPG", jpeg(6), 3),
    ("CameraRollDomain", "Media/PhotoData/Photos.sqlite", photos, 3),
    ("HomeDomain", "Library/SMS/sms.db", sms, 3),
    ("MediaDomain", "Library/SMS/Attachments/ab/11/ABC/IMG_7.jpeg", jpeg(7), 3),
    ("MediaDomain", "Library/SMS/Attachments/cd/02/XYZ/IMG_9.jpeg", jpeg(8), 3),
    ("AppDomainGroup-group.net.whatsapp.WhatsApp.shared", "Message/Media/1234@s.whatsapp.net/a/b/photo.jpg", jpeg(9), 3),
    ("AppDomainGroup-group.com.apple.VoiceMemos.shared", "Recordings/20240101 120000.m4a", b"\0\0\0\x18ftypM4A " + b"a" * 4000, 3),
    ("AppDomainGroup-group.com.apple.VoiceMemos.shared", "Recordings/CloudRecordings.db", voice, 3),
    ("AppDomain-com.example.app", "Library/Caches/junk.jpg", jpeg(3), 3),
    ("AppDomain-com.example.app", "Documents/report.pdf", b"%PDF-1.4 hello", 3),
    ("KeychainDomain", "keychain-backup.plist", b"x", 3),
]
if encrypted:  # a file only the device itself could open (class 5 has no passcode wrap)
    files.append(("CameraRollDomain", "Media/DCIM/100APPLE/IMG_0009.JPG", jpeg(5), 5))

# Keybag: iOS 10.2+ style (DPSL/DPIC, then SALT/ITER), with few rounds for speed.
dpsl, salt = os.urandom(20), os.urandom(20)
dpic, it = 1000, 1000
pk = pbk("sha1", pbk("sha256", PW, dpsl, dpic), salt, it)
classkeys = {c: os.urandom(32) for c in (1, 2, 3, 4, 5)}


def tlv(tag, val):
    if isinstance(val, int):
        val = struct.pack(">I", val)
    return tag + struct.pack(">I", len(val)) + val


bag = b"".join([tlv(b"VERS", 4), tlv(b"TYPE", 1), tlv(b"UUID", os.urandom(16)), tlv(b"HMCK", os.urandom(40)),
                tlv(b"WRAP", 0), tlv(b"SALT", salt), tlv(b"ITER", it), tlv(b"DPWT", 1), tlv(b"DPIC", dpic),
                tlv(b"DPSL", dpsl)])
for cl, k in classkeys.items():
    wrap = 1 if cl == 5 else 2  # class 5 needs the device's own key
    wrapped = aes_key_wrap(pk if wrap == 2 else os.urandom(32), k)
    bag += b"".join([tlv(b"UUID", os.urandom(16)), tlv(b"CLAS", cl), tlv(b"WRAP", wrap), tlv(b"KTYP", 0),
                     tlv(b"WPKY", wrapped)])

db_path = os.path.join(out, "Manifest.db")
if os.path.exists(db_path):
    os.remove(db_path)
db = sqlite3.connect(db_path)
db.execute("CREATE TABLE Files (fileID TEXT PRIMARY KEY, domain TEXT, relativePath TEXT, flags INTEGER, file BLOB)")
db.execute("CREATE TABLE Properties (key TEXT PRIMARY KEY, value BLOB)")
for domain, rel, data, cl in files:
    fid = hashlib.sha1(f"{domain}-{rel}".encode()).hexdigest()
    obj = {"$class": plistlib.UID(3), "Size": len(data), "LastModified": 1700000000, "ProtectionClass": cl,
           "RelativePath": plistlib.UID(2), "Flags": 1}
    objects = ["$null", obj, rel, {"$classname": "MBFile", "$classes": ["MBFile", "NSObject"]}]
    stored = data
    if encrypted:
        fk = os.urandom(32)
        obj["EncryptionKey"] = plistlib.UID(4)
        objects.append({"NS.data": struct.pack("<I", cl) + aes_key_wrap(classkeys[cl], fk), "$class": plistlib.UID(3)})
        stored = enc(fk, data)
    blob = plistlib.dumps({"$version": 100000, "$archiver": "NSKeyedArchiver", "$top": {"root": plistlib.UID(1)},
                           "$objects": objects}, fmt=plistlib.FMT_BINARY)
    db.execute("INSERT INTO Files VALUES (?,?,?,1,?)", (fid, domain, rel, blob))
    os.makedirs(os.path.join(out, fid[:2]), exist_ok=True)
    open(os.path.join(out, fid[:2], fid), "wb").write(stored)
db.execute("INSERT INTO Files VALUES ('dir1','CameraRollDomain','Media/DCIM',2,NULL)")
db.commit()
db.close()

manifest = {"IsEncrypted": encrypted, "Version": "10.0"}
if encrypted:
    mkey = os.urandom(32)
    manifest["BackupKeyBag"] = bag
    manifest["ManifestKey"] = struct.pack("<I", 4) + aes_key_wrap(classkeys[4], mkey)
    raw = open(db_path, "rb").read()
    open(db_path, "wb").write(enc(mkey, raw))
plistlib.dump(manifest, open(os.path.join(out, "Manifest.plist"), "wb"), fmt=plistlib.FMT_BINARY)
plistlib.dump({"Device Name": "Test iPhone", "Product Type": "iPhone15,2", "Product Version": "17.5",
               "Last Backup Date": datetime.datetime(2026, 3, 12, 10, 0)},
              open(os.path.join(out, "Info.plist"), "wb"))
print("ok", out)
