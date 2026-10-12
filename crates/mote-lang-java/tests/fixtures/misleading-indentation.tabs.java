class Tabs {
	void run(boolean a) {
		if (a)
			first();
			second();	// expect: misleading-indentation
		if (a)
			first();
		third();
		if (a)
			first();
            mixed();
	}
}
