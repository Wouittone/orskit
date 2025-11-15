import org.gradle.api.tasks.testing.logging.TestLogEvent

plugins {
    `java-library`
    jacoco
    id("com.gradle.common-custom-user-data-gradle-plugin") version "1.13"
}

java {
    sourceCompatibility = JavaVersion.VERSION_25
    targetCompatibility = JavaVersion.VERSION_25
    toolchain {
        languageVersion = JavaLanguageVersion.of(25)
    }
}

repositories {
    mavenCentral()
}

dependencies {
    // Add Java dependencies here as needed
    // Example for testing
    testImplementation("org.junit.jupiter:junit-jupiter:5.11.0")
}

// Task to build Rust library
tasks.register<Exec>("buildRustLib") {
    description = "Build the Rust library with Cargo"
    commandLine("cargo", "build", "--release")
    workingDir = projectDir
}

// Task to clean Rust build artifacts
tasks.register<Exec>("cleanRustLib") {
    description = "Clean Rust build artifacts"
    commandLine("cargo", "clean")
    workingDir = projectDir
}

// Copy Rust binary to resources
tasks.register<Copy>("copyRustLib") {
    description = "Copy compiled Rust library to build resources"
    dependsOn("buildRustLib")
    from("target/release") {
        include("*.so", "*.dylib", "*.dll", "*.dll.a")
    }
    into("build/resources/main")
}

// Ensure Rust lib is built before Java compilation
tasks.named("compileJava") {
    dependsOn("buildRustLib")
}

// Process resources depends on copying Rust library
tasks.named("processResources") {
    dependsOn("copyRustLib")
}

// Test configuration
tasks.test {
    useJUnitPlatform()
    testLogging {
        events(TestLogEvent.PASSED, TestLogEvent.SKIPPED, TestLogEvent.FAILED)
        exceptionFormat = org.gradle.api.tasks.testing.logging.TestExceptionFormat.FULL
        showStandardStreams = false
    }
}

// Java compilation configuration
tasks.compileJava {
    options.apply {
        encoding = "UTF-8"
        isDeprecation = true
        isWarnings = true
        compilerArgs.addAll(listOf(
            "-Xlint:all",
            "-Xlint:-processing",
            "--enable-preview"
        ))
    }
}

// JAR task configuration
tasks.jar {
    manifest {
        attributes(
            "Implementation-Title" to project.name,
            "Implementation-Version" to project.version,
            "Implementation-Vendor" to "Wouittone",
            "Specification-Title" to "orskit-jni",
            "Specification-Version" to project.version,
            "Bundle-License" to "MIT OR Apache-2.0"
        )
    }
}

// Code coverage with JaCoCo
jacoco {
    toolVersion = "0.8.12"
}

tasks.test {
    finalizedBy("jacocoTestReport")
}

tasks.jacocoTestReport {
    dependsOn("test")
}
