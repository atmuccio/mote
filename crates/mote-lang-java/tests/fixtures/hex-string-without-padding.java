class Digests {
    String wrong(byte[] digest, byte[] ip) {
        StringBuilder hex = new StringBuilder();
        for (byte b : digest) {
            hex.append(Integer.toHexString(b & 0xFF));                  // expect: hex-string-without-padding
        }
        String id = "";
        for (int i = 0; i < ip.length; i++) {
            id += Integer.toHexString((ip[i]) & 0xff);                  // expect: hex-string-without-padding
        }
        StringBuffer upper = new StringBuffer();
        for (byte b : digest) {
            upper.append(Integer.toHexString(0xff & b).toUpperCase());  // expect: hex-string-without-padding
        }
        return hex + id + upper;
    }

    String fine(byte[] digest, byte b0, int port) {
        StringBuilder a = new StringBuilder();
        for (byte b : digest) {
            String h = Integer.toHexString(b & 0xFF);
            if (h.length() == 1) {
                a.append('0');
            }
            a.append(h);
        }
        StringBuilder mac = new StringBuilder();
        for (byte b : digest) {
            mac.append(Integer.toHexString(b & 0xFF)).append(':');
        }
        StringBuilder c = new StringBuilder();
        for (byte b : digest) {
            c.append(String.format("%02x", b));
        }
        StringBuilder d = new StringBuilder();
        for (byte b : digest) {
            d.append(Integer.toHexString((b & 0xff) + 0x100).substring(1));
        }
        String single = "byte " + Integer.toHexString(b0 & 0xff);
        String p = "port " + Integer.toHexString(port);
        StringBuilder e = new StringBuilder();
        for (int v : new int[] {1, 2}) {
            e.append(Integer.toHexString(v));
        }
        return a + single + p;
    }
}
